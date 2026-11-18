# NFS (experimental)

An NFS connection opens as a file browser that fills the whole tab, like [SMB](smb.md). It speaks NFS version 3 only. It is experimental: it is the least tested of the file protocols.

Choose the protocol **NFS** in the connection's properties, or type `nfs://host` in the new-tab box. The host is the server's name or address. The default port, 111, is the portmapper's, which tells the app where the NFS services are.

## Exports

The top of the browser lists the **exports** the server offers. An export such as `/srv/data` shows as the folders `srv`, then `data`; only the last one is real, the ones above it just lead there. Open an export to browse it.

## Who you are

NFS has no passwords. The server trusts the **user and group number** the client sends, so the app sends the ones you choose:

- Put them as the **user name** of the connection's credential: `1000` (the group is then 1000 too) or `1000:100`. The credential's kind does not matter and nothing is asked.
- With no credential, or a name that is not numbers, the app is `nobody` (65534), which can usually read what is world-readable and write nothing.

Everything crosses the network in the clear, and anyone who can reach the server can claim any number. Use NFS on a network you trust, or through a tunnel.

## The server must allow it

Most Linux servers only accept NFS clients that connect from a **privileged port** (below 1024). A desktop app cannot open one without being administrator, so it connects from an ordinary port. On the server, add the **`insecure`** option to the export in `/etc/exports`:

```
/srv/data  192.168.1.0/24(rw,sync,insecure,no_subtree_check)
```

Then run `exportfs -ra`. Without it the connection is refused, or the listing fails with a permission error. Servers that do not insist on privileged ports, such as NFS-Ganesha with its default settings, need nothing.

## Files

The file actions are the same as for [SFTP](ssh.md#files): upload, download, folders, preview, rename, delete, new folder, copy path, and the transfer queue with cancel and retry.

- **Resume.** A retry carries on from what is already there.
- **Writes are synchronous.** Each block is on the server's disk before the next is sent, so a server that restarts halfway cannot lose data it reported as received. Large uploads are slower for it.
- **Rename** never replaces a name that is taken.
- **Links** show as links and are not followed.

## Not available yet

- NFS 4 and NFS 2.
- Kerberos (`sec=krb5`), and groups beyond the one you name.
- Moving a file to another export by renaming it.
