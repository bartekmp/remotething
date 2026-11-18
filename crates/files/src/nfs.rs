//! NFS version 3 exports (experimental). The root of the server lists its exports as folders,
//! with the folders above an export such as `/srv/data` made up: `/` holds `srv`, `/srv`
//! holds `data`, and `/srv/data` is the export itself.
//!
//! NFS has no passwords. The server trusts the user and group number the client sends, so
//! those are settings. Many servers only take clients on a privileged source port, which an
//! ordinary program cannot use; such a server needs the `insecure` option on its export.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::path::Path;

use async_trait::async_trait;
use nfs3_client::net::Connector;
use nfs3_client::nfs3_types::nfs3::{
    self, CREATE3args, GETATTR3args, LOOKUP3args, MKDIR3args, Nfs3Option, Nfs3Result, READ3args,
    READDIRPLUS3args, REMOVE3args, RENAME3args, RMDIR3args, WRITE3args, createhow3, diropargs3,
    fattr3, filename3, ftype3, nfs_fh3, nfsstat3, sattr3, stable_how,
};
use nfs3_client::nfs3_types::portmap::IPPROTO_TCP;
use nfs3_client::nfs3_types::rpc::{auth_unix, opaque_auth};
use nfs3_client::nfs3_types::xdr_codec::Opaque;
use nfs3_client::tokio::{TokioConnector, TokioIo};
use nfs3_client::{MountClient, Nfs3Connection, Nfs3ConnectionBuilder, PortmapperClient, RpcError};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::entry::{Entry, EntryKind, sort};
use crate::fs::{OnProgress, RemoteFs};
use crate::transfer::{CHUNK, Progress, Transferred};
use crate::{Error, Result};

/// Runs `$body`, a block that may use `.await`, with the mounted export that holds `$path`
/// and the path inside it; `$export` names the export.
macro_rules! in_export {
    ($self:expr, $action:expr, $path:expr, $mount:ident, $rest:ident, $export:ident => $body:block) => {{
        let Place::In {
            export: $export,
            rest: $rest,
        } = locate(&$self.exports, $path)
        else {
            return Err(failed($action, $path, "that is not inside an export"));
        };
        let mut mounts = $self.mounts.lock().await;
        let $mount = $self.mounted(&mut mounts, $export, $path).await?;
        $body
    }};
}

/// The user and group sent when the connection does not name any: "nobody".
pub const NOBODY: u32 = 65534;

/// One export, mounted: its connection and its root.
type Mount = Nfs3Connection<TokioIo<TcpStream>>;

/// Where to connect and who to be.
#[derive(Clone)]
pub struct NfsSettings {
    /// A host name or an IP address.
    pub host: String,
    /// The port of the portmapper, usually 111. It tells where the other services are.
    pub port: u16,
    /// The user number the server sees.
    pub uid: u32,
    /// The group number the server sees.
    pub gid: u32,
}

/// The files of an NFS server.
pub struct NfsFs {
    address: SocketAddr,
    credential: opaque_auth<'static>,
    /// The exports the server offers, as it wrote them (`/srv/data`).
    exports: Vec<String>,
    /// The exports that are mounted so far.
    mounts: Mutex<HashMap<String, Mount>>,
}

impl NfsFs {
    /// Reaches the server and asks which exports it offers.
    pub async fn connect(settings: NfsSettings) -> Result<Self> {
        let address = tokio::net::lookup_host((settings.host.as_str(), settings.port))
            .await
            .map_err(|error| Error::Unavailable(error.to_string()))?
            .next()
            .ok_or_else(|| Error::Unavailable("the host name has no address".to_string()))?;
        let credential = opaque_auth::auth_unix(&auth_unix {
            stamp: 0,
            machinename: Opaque::borrowed(b"remotething"),
            uid: settings.uid,
            gid: settings.gid,
            gids: Vec::new(),
        });
        let exports = list_exports(address, &credential)
            .await
            .map_err(|error| Error::Unavailable(error.to_string()))?;
        Ok(Self {
            address,
            credential,
            exports,
            mounts: Mutex::new(HashMap::new()),
        })
    }

    /// The mounted export, mounting it the first time.
    async fn mounted<'a>(
        &self,
        mounts: &'a mut HashMap<String, Mount>,
        export: &str,
        path: &str,
    ) -> Result<&'a mut Mount> {
        if !mounts.contains_key(export) {
            let mount =
                Nfs3ConnectionBuilder::new(TokioConnector, self.address.ip().to_string(), export)
                    .portmapper_port(self.address.port())
                    .credential(self.credential.clone())
                    .connect_from_privileged_port(false)
                    .mount()
                    .await
                    .map_err(|error| failed("cannot open", path, error))?;
            mounts.insert(export.to_string(), mount);
        }
        mounts
            .get_mut(export)
            .ok_or_else(|| failed("cannot open", path, "the export is not mounted"))
    }

    /// The export that holds `path`, and the file handle of `path` in it.
    async fn open(&self, action: &'static str, path: &str) -> Result<(String, nfs_fh3)> {
        in_export!(self, action, path, mount, rest, export => {
            match walk(mount, rest.as_str(), path).await? {
                Some((handle, _)) => Ok((export.to_string(), handle)),
                None => Err(failed(action, path, "no such file or folder")),
            }
        })
    }

    /// Reads a block of a file. Fewer bytes than asked for may come back.
    async fn read_block(
        &self,
        export: &str,
        file: &nfs_fh3,
        offset: u64,
        count: u32,
        path: &str,
    ) -> Result<(Vec<u8>, bool)> {
        let mut mounts = self.mounts.lock().await;
        let mount = self.mounted(&mut mounts, export, path).await?;
        let reply = mount
            .read(&READ3args {
                file: file.clone(),
                offset,
                count,
            })
            .await;
        let ok = unwrap(reply, "cannot read", path)?;
        Ok((ok.data.as_ref().to_vec(), ok.eof))
    }

    /// Writes all of `data` at `offset`, however many tries the server needs.
    async fn write_all_at(
        &self,
        export: &str,
        file: &nfs_fh3,
        mut offset: u64,
        mut data: &[u8],
        path: &str,
    ) -> Result<()> {
        while !data.is_empty() {
            let mut mounts = self.mounts.lock().await;
            let mount = self.mounted(&mut mounts, export, path).await?;
            let reply = mount
                .write(&WRITE3args {
                    file: file.clone(),
                    offset,
                    count: data.len() as u32,
                    // Each block is on the disk of the server before it answers, so a server
                    // that restarts halfway cannot lose what was reported as sent.
                    stable: stable_how::FILE_SYNC,
                    data: Opaque::borrowed(data),
                })
                .await;
            let written = unwrap(reply, "cannot upload", path)?.count as usize;
            if written == 0 {
                return Err(failed("cannot upload", path, "the server wrote nothing"));
            }
            let written = written.min(data.len());
            offset += written as u64;
            data = &data[written..];
        }
        Ok(())
    }
}

/// Asks the mount service for the exports.
async fn list_exports(
    address: SocketAddr,
    credential: &opaque_auth<'static>,
) -> std::result::Result<Vec<String>, String> {
    let io = TokioConnector
        .connect(address)
        .await
        .map_err(|error| error.to_string())?;
    let mut portmapper = PortmapperClient::new(io);
    let port = portmapper
        .getport(
            nfs3_client::nfs3_types::mount::PROGRAM,
            nfs3_client::nfs3_types::mount::VERSION,
            IPPROTO_TCP,
        )
        .await
        .map_err(|error| error.to_string())?;
    let io = TokioConnector
        .connect(SocketAddr::new(address.ip(), port))
        .await
        .map_err(|error| error.to_string())?;
    let mut mount = MountClient::new_with_auth(io, credential.clone(), opaque_auth::default());
    let exports = mount.export().await.map_err(|error| error.to_string())?;
    Ok(exports
        .into_inner()
        .into_iter()
        .map(|export| String::from_utf8_lossy(export.ex_dir.0.as_ref()).into_owned())
        .collect())
}

/// Where a path of the browser is on the server.
#[derive(Debug, PartialEq, Eq)]
enum Place<'a> {
    /// Above the exports, in a folder that only exists to lead to them: the names below it.
    Above(Vec<String>),
    /// In an export, at this path inside it (empty for the export's own top).
    In { export: &'a str, rest: String },
    /// Nowhere on the server.
    Nowhere,
}

/// Finds a path among the exports. The longest export that holds the path wins.
fn locate<'a>(exports: &'a [String], path: &str) -> Place<'a> {
    let wanted = path.trim_matches('/');
    let mut best: Option<(&str, &str)> = None;
    for export in exports {
        let top = export.trim_matches('/');
        let rest = if top.is_empty() {
            Some(wanted)
        } else {
            wanted
                .strip_prefix(top)
                .and_then(|rest| rest.strip_prefix('/').or(rest.is_empty().then_some(rest)))
        };
        if let Some(rest) = rest
            && best.is_none_or(|(known, _)| top.len() > known.trim_matches('/').len())
        {
            best = Some((export, rest));
        }
    }
    if let Some((export, rest)) = best {
        return Place::In {
            export,
            rest: rest.to_string(),
        };
    }
    let below: BTreeSet<String> = exports
        .iter()
        .filter_map(|export| {
            let top = export.trim_matches('/');
            let rest = if wanted.is_empty() {
                top
            } else {
                top.strip_prefix(wanted)?.strip_prefix('/')?
            };
            rest.split('/').next().map(str::to_string)
        })
        .collect();
    if below.is_empty() {
        Place::Nowhere
    } else {
        Place::Above(below.into_iter().collect())
    }
}

fn failed(action: &'static str, path: &str, error: impl std::fmt::Display) -> Error {
    Error::Remote {
        action,
        path: path.to_string(),
        reason: error.to_string(),
    }
}

fn local(path: &Path, error: std::io::Error) -> Error {
    Error::Local {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

/// The answer of a call, or why there is none: the call itself failed, or the server said no.
fn unwrap<T, E>(
    reply: std::result::Result<Nfs3Result<T, E>, RpcError>,
    action: &'static str,
    path: &str,
) -> Result<T> {
    match reply.map_err(|error| failed(action, path, error))? {
        Nfs3Result::Ok(ok) => Ok(ok),
        Nfs3Result::Err((status, _)) => Err(failed(action, path, status)),
    }
}

fn name(text: &str) -> filename3<'_> {
    filename3(Opaque::borrowed(text.as_bytes()))
}

fn attributes(option: nfs3::post_op_attr) -> Option<fattr3> {
    match option {
        Nfs3Option::Some(attributes) => Some(attributes),
        Nfs3Option::None => None,
    }
}

/// Splits `a/b/c` into the folder (`a/b`) and the name (`c`).
fn split_last(rest: &str) -> (&str, &str) {
    rest.rsplit_once('/').unwrap_or(("", rest))
}

/// Follows `rest` from the top of the export. `None` when something on the way is missing.
async fn walk(
    mount: &mut Mount,
    rest: &str,
    path: &str,
) -> Result<Option<(nfs_fh3, Option<fattr3>)>> {
    let mut handle = mount.root_nfs_fh3();
    let mut found = None;
    for part in rest.split('/').filter(|part| !part.is_empty()) {
        let reply = mount
            .lookup(&LOOKUP3args {
                what: diropargs3 {
                    dir: handle.clone(),
                    name: name(part),
                },
            })
            .await
            .map_err(|error| failed("cannot look at", path, error))?;
        match reply {
            Nfs3Result::Ok(ok) => {
                handle = ok.object;
                found = attributes(ok.obj_attributes);
            }
            Nfs3Result::Err((nfsstat3::NFS3ERR_NOENT, _)) => return Ok(None),
            Nfs3Result::Err((status, _)) => return Err(failed("cannot look at", path, status)),
        }
    }
    Ok(Some((handle, found)))
}

/// What the server knows about a file, asking for it when the walk did not bring it.
async fn attributes_of(
    mount: &mut Mount,
    handle: nfs_fh3,
    known: Option<fattr3>,
    path: &str,
) -> Result<fattr3> {
    if let Some(known) = known {
        return Ok(known);
    }
    let reply = mount.getattr(&GETATTR3args { object: handle }).await;
    Ok(unwrap(reply, "cannot look at", path)?.obj_attributes)
}

fn kind_of(kind: ftype3) -> EntryKind {
    match kind {
        ftype3::NF3DIR => EntryKind::Folder,
        ftype3::NF3REG => EntryKind::File,
        ftype3::NF3LNK => EntryKind::Link,
        _ => EntryKind::Other,
    }
}

#[async_trait]
impl RemoteFs for NfsFs {
    async fn home(&self) -> Result<String> {
        Ok("/".to_string())
    }

    async fn list(&self, folder: &str) -> Result<Vec<Entry>> {
        let place = locate(&self.exports, folder);
        let (export, rest) = match place {
            Place::Above(names) => {
                return Ok(names
                    .into_iter()
                    .map(|name| Entry {
                        name,
                        kind: EntryKind::Folder,
                        size: None,
                        modified: None,
                        permissions: None,
                    })
                    .collect());
            }
            Place::Nowhere => return Err(failed("cannot list", folder, "no such folder")),
            Place::In { export, rest } => (export, rest),
        };
        let mut mounts = self.mounts.lock().await;
        let mount = self.mounted(&mut mounts, export, folder).await?;
        let Some((directory, _)) = walk(mount, &rest, folder).await? else {
            return Err(failed("cannot list", folder, "no such folder"));
        };

        let mut entries = Vec::new();
        let mut cookie = nfs3::cookie3::default();
        let mut verifier = nfs3::cookieverf3::default();
        loop {
            let reply = mount
                .readdirplus(&READDIRPLUS3args {
                    dir: directory.clone(),
                    cookie,
                    cookieverf: verifier,
                    maxcount: 128 * 1024,
                    dircount: 128 * 1024,
                })
                .await;
            let page = unwrap(reply, "cannot list", folder)?;
            let listed = page.reply.entries.into_inner();
            if let Some(last) = listed.last() {
                cookie = last.cookie;
            }
            verifier = page.cookieverf;
            for item in &listed {
                let text = String::from_utf8_lossy(item.name.0.as_ref()).into_owned();
                if text == "." || text == ".." {
                    continue;
                }
                let attributes = match &item.name_attributes {
                    Nfs3Option::Some(attributes) => Some(attributes),
                    Nfs3Option::None => None,
                };
                entries.push(Entry {
                    name: text,
                    kind: attributes.map_or(EntryKind::Other, |a| kind_of(a.type_)),
                    size: attributes.map(|a| a.size),
                    modified: attributes.map(|a| u64::from(a.mtime.seconds)),
                    permissions: attributes.map(|a| a.mode & 0o7777),
                });
            }
            if page.reply.eof || listed.is_empty() {
                break;
            }
        }
        sort(&mut entries);
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let place = locate(&self.exports, path);
        if let Place::Above(_) = place {
            return Ok(true);
        }
        if place == Place::Nowhere {
            return Ok(false);
        }
        in_export!(self, "cannot look at", path, mount, rest, export => {
            Ok(walk(mount, rest.as_str(), path).await?.is_some())
        })
    }

    async fn size(&self, path: &str) -> Result<u64> {
        in_export!(self, "cannot look at", path, mount, rest, export => {
            let Some((handle, known)) = walk(mount, rest.as_str(), path).await? else {
                return Err(failed("cannot look at", path, "no such file"));
            };
            Ok(attributes_of(mount, handle, known, path).await?.size)
        })
    }

    async fn create_folder(&self, path: &str) -> Result<()> {
        in_export!(self, "cannot create", path, mount, rest, export => {
            let (above, last) = split_last(rest.as_str());
            let Some((directory, _)) = walk(mount, above, path).await? else {
                return Err(failed("cannot create", path, "the folder above is missing"));
            };
            let reply = mount
                .mkdir(&MKDIR3args {
                    where_: diropargs3 { dir: directory, name: name(last) },
                    attributes: sattr3 {
                        mode: Nfs3Option::Some(0o755),
                        ..sattr3::default()
                    },
                })
                .await;
            unwrap(reply, "cannot create", path).map(|_| ())
        })
    }

    async fn create_folders(&self, folder: &str) -> Result<()> {
        let mut path = String::new();
        for part in folder.split('/').filter(|part| !part.is_empty()) {
            path.push('/');
            path.push_str(part);
            // Folders above the export are only there to lead to it, and the export is there.
            let inside =
                matches!(locate(&self.exports, &path), Place::In { rest, .. } if !rest.is_empty());
            if inside && !self.exists(&path).await? {
                self.create_folder(&path).await?;
            }
        }
        Ok(())
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        // The server would replace a file that is in the way, so look first.
        if self.exists(to).await? {
            return Err(failed("cannot rename", from, "the new name is taken"));
        }
        let (
            Place::In {
                export,
                rest: from_rest,
            },
            Place::In {
                export: target,
                rest: to_rest,
            },
        ) = (locate(&self.exports, from), locate(&self.exports, to))
        else {
            return Err(failed("cannot rename", from, "exports cannot be renamed"));
        };
        if export != target {
            return Err(failed(
                "cannot rename",
                from,
                "it cannot move to another export",
            ));
        }
        let mut mounts = self.mounts.lock().await;
        let mount = self.mounted(&mut mounts, export, from).await?;
        let (from_above, from_last) = split_last(&from_rest);
        let (to_above, to_last) = split_last(&to_rest);
        let Some((from_directory, _)) = walk(mount, from_above, from).await? else {
            return Err(failed("cannot rename", from, "no such file or folder"));
        };
        let Some((to_directory, _)) = walk(mount, to_above, to).await? else {
            return Err(failed("cannot rename", to, "the folder above is missing"));
        };
        let reply = mount
            .rename(&RENAME3args {
                from: diropargs3 {
                    dir: from_directory,
                    name: name(from_last),
                },
                to: diropargs3 {
                    dir: to_directory,
                    name: name(to_last),
                },
            })
            .await;
        unwrap(reply, "cannot rename", from).map(|_| ())
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        in_export!(self, "cannot delete", path, mount, rest, export => {
            let (above, last) = split_last(rest.as_str());
            let Some((directory, _)) = walk(mount, above, path).await? else {
                return Err(failed("cannot delete", path, "no such file"));
            };
            let reply = mount
                .remove(&REMOVE3args { object: diropargs3 { dir: directory, name: name(last) } })
                .await;
            unwrap(reply, "cannot delete", path).map(|_| ())
        })
    }

    async fn remove_folder(&self, path: &str) -> Result<()> {
        in_export!(self, "cannot delete", path, mount, rest, export => {
            let (above, last) = split_last(rest.as_str());
            let Some((directory, _)) = walk(mount, above, path).await? else {
                return Err(failed("cannot delete", path, "no such folder"));
            };
            let reply = mount
                .rmdir(&RMDIR3args { object: diropargs3 { dir: directory, name: name(last) } })
                .await;
            unwrap(reply, "cannot delete", path).map(|_| ())
        })
    }

    async fn read_start(&self, path: &str, limit: u64) -> Result<(Vec<u8>, bool)> {
        let (export, file) = self.open("cannot open", path).await?;
        let mut bytes = Vec::new();
        while (bytes.len() as u64) <= limit {
            let wanted = (limit + 1 - bytes.len() as u64).min(CHUNK as u64) as u32;
            let (block, end) = self
                .read_block(&export, &file, bytes.len() as u64, wanted, path)
                .await?;
            bytes.extend(block);
            if end {
                break;
            }
        }
        let longer = bytes.len() as u64 > limit;
        bytes.truncate(limit as usize);
        Ok((bytes, longer))
    }

    async fn download_tracked(
        &self,
        remote_file: &str,
        local_file: &Path,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        let (export, file) = self.open("cannot open", remote_file).await?;
        let total = self.size(remote_file).await?;
        let offset = if resume {
            match tokio::fs::metadata(local_file).await {
                Ok(metadata) if total > 0 && metadata.len() <= total => metadata.len(),
                _ => 0,
            }
        } else {
            0
        };
        if let Some(folder) = local_file.parent() {
            tokio::fs::create_dir_all(folder)
                .await
                .map_err(|error| local(folder, error))?;
        }
        let mut options = tokio::fs::OpenOptions::new();
        if offset > 0 {
            options.append(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        let mut target = options
            .open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;

        let mut done = offset;
        progress(Progress { done, total });
        while done < total {
            let wanted = (total - done).min(CHUNK as u64) as u32;
            let (block, end) = self
                .read_block(&export, &file, done, wanted, remote_file)
                .await?;
            if block.is_empty() {
                break;
            }
            target
                .write_all(&block)
                .await
                .map_err(|error| local(local_file, error))?;
            done += block.len() as u64;
            progress(Progress { done, total });
            if end {
                break;
            }
        }
        target
            .flush()
            .await
            .map_err(|error| local(local_file, error))?;
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }

    async fn upload_tracked(
        &self,
        local_file: &Path,
        remote_file: &str,
        resume: bool,
        progress: OnProgress<'_>,
    ) -> Result<Transferred> {
        let mut source = tokio::fs::File::open(local_file)
            .await
            .map_err(|error| local(local_file, error))?;
        let total = source
            .metadata()
            .await
            .map_err(|error| local(local_file, error))?
            .len();
        let offset = if resume {
            match self.size(remote_file).await {
                Ok(size) if size > 0 && size <= total => size,
                _ => 0,
            }
        } else {
            0
        };
        if let Some(folder) = crate::parent(remote_file) {
            self.create_folders(&folder).await?;
        }
        let (export, file) = if offset > 0 {
            self.open("cannot open", remote_file).await?
        } else {
            self.create_empty(remote_file).await?
        };
        if offset > 0 {
            source
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|error| local(local_file, error))?;
        }

        let mut done = offset;
        progress(Progress { done, total });
        let mut buffer = vec![0u8; CHUNK];
        loop {
            let read = source
                .read(&mut buffer)
                .await
                .map_err(|error| local(local_file, error))?;
            if read == 0 {
                break;
            }
            self.write_all_at(&export, &file, done, &buffer[..read], remote_file)
                .await?;
            done += read as u64;
            progress(Progress {
                done,
                total: total.max(done),
            });
        }
        Ok(Transferred {
            bytes: done,
            resumed_from: offset,
        })
    }
}

impl NfsFs {
    /// Makes a file with nothing in it, or empties the one that is there.
    async fn create_empty(&self, path: &str) -> Result<(String, nfs_fh3)> {
        in_export!(self, "cannot create", path, mount, rest, export => {
            let (above, last) = split_last(rest.as_str());
            let Some((directory, _)) = walk(mount, above, path).await? else {
                return Err(failed("cannot create", path, "the folder above is missing"));
            };
            let reply = mount
                .create(&CREATE3args {
                    where_: diropargs3 { dir: directory.clone(), name: name(last) },
                    how: createhow3::UNCHECKED(sattr3 {
                        mode: Nfs3Option::Some(0o644),
                        size: Nfs3Option::Some(0),
                        ..sattr3::default()
                    }),
                })
                .await;
            let created = unwrap(reply, "cannot create", path)?;
            let handle = match created.obj {
                Nfs3Option::Some(handle) => handle,
                // The server did not hand the new file over, so ask for it by name.
                Nfs3Option::None => {
                    let found = walk(mount, rest.as_str(), path).await?;
                    found.map(|(handle, _)| handle).ok_or_else(|| {
                        failed("cannot create", path, "the new file is not there")
                    })?
                }
            };
            Ok((export.to_string(), handle))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exports(list: &[&str]) -> Vec<String> {
        list.iter().map(|export| export.to_string()).collect()
    }

    #[test]
    fn the_root_leads_to_the_exports_through_made_up_folders() {
        let list = exports(&["/srv/data", "/srv/media/photos", "/home"]);

        assert_eq!(
            locate(&list, "/"),
            Place::Above(vec!["home".to_string(), "srv".to_string()])
        );
        assert_eq!(
            locate(&list, "/srv"),
            Place::Above(vec!["data".to_string(), "media".to_string()])
        );
        assert_eq!(
            locate(&list, "/srv/media"),
            Place::Above(vec!["photos".to_string()])
        );
        assert_eq!(locate(&list, "/etc"), Place::Nowhere);
        // A name that only starts like a folder on the way is not that folder.
        assert_eq!(locate(&list, "/sr"), Place::Nowhere);
    }

    #[test]
    fn a_path_inside_an_export_splits_into_the_export_and_the_rest() {
        let list = exports(&["/srv/data", "/home"]);

        assert_eq!(
            locate(&list, "/srv/data"),
            Place::In {
                export: "/srv/data",
                rest: String::new()
            }
        );
        assert_eq!(
            locate(&list, "/srv/data/docs/a.txt"),
            Place::In {
                export: "/srv/data",
                rest: "docs/a.txt".to_string()
            }
        );
        // `/srv/database` is not inside `/srv/data`.
        assert_eq!(locate(&list, "/srv/database"), Place::Nowhere);
    }

    #[test]
    fn the_longest_export_wins_and_an_export_of_the_root_holds_everything() {
        let nested = exports(&["/srv", "/srv/data"]);
        assert_eq!(
            locate(&nested, "/srv/data/x"),
            Place::In {
                export: "/srv/data",
                rest: "x".to_string()
            }
        );
        assert_eq!(
            locate(&nested, "/srv/other/x"),
            Place::In {
                export: "/srv",
                rest: "other/x".to_string()
            }
        );

        let whole = exports(&["/"]);
        assert_eq!(
            locate(&whole, "/a/b"),
            Place::In {
                export: "/",
                rest: "a/b".to_string()
            }
        );
        assert_eq!(
            locate(&whole, "/"),
            Place::In {
                export: "/",
                rest: String::new()
            }
        );
    }

    #[test]
    fn the_last_part_of_a_path_is_split_off() {
        assert_eq!(split_last("a/b/c"), ("a/b", "c"));
        assert_eq!(split_last("c"), ("", "c"));
    }
}
