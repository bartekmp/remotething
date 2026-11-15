// Every stage is one `cargo xtask` call inside ci/Dockerfile.
pipeline {
    agent {
        dockerfile {
            dir 'ci'
            // 1 = the Microsoft SDK licence is accepted (see ci/Dockerfile).
            additionalBuildArgs '--build-arg XWIN_ACCEPT_LICENSE=1'
            // Root inside the container, with caches on named volumes outside the workspace.
            args '-u root -v remotething-cargo-registry:/usr/local/cargo/registry -v remotething-cache:/cache'
        }
    }
    environment {
        CARGO_TARGET_DIR = '/cache/target'
        XWIN_CACHE_DIR = '/cache/xwin'
    }
    stages {
        stage('Version') {
            // A pull request that changes the crates must raise the version.
            when { changeRequest() }
            steps {
                // The workspace belongs to the Jenkins user; git inside the container runs as root.
                sh 'git config --global --add safe.directory "*"'
                // The repository is private; the token goes in a header so it never appears in a URL or the log.
                withCredentials([usernamePassword(credentialsId: 'github-credentials-token', usernameVariable: 'GIT_USER', passwordVariable: 'GIT_TOKEN')]) {
                    sh '''
                        set +x
                        AUTH=$(printf '%s:%s' "$GIT_USER" "$GIT_TOKEN" | base64 -w0)
                        git -c "http.extraHeader=Authorization: Basic $AUTH" fetch --no-tags origin "+refs/heads/${CHANGE_TARGET}:refs/remotes/origin/${CHANGE_TARGET}"
                    '''
                }
                sh 'cargo xtask version-check "origin/${CHANGE_TARGET}"'
            }
        }
        stage('Lint') {
            steps { sh 'cargo xtask lint' }
        }
        stage('Unit tests') {
            steps { sh 'cargo xtask test' }
        }
        stage('Build Linux') {
            steps { sh 'cargo xtask build linux' }
        }
        stage('Build Windows') {
            // The slowest stage. Pull requests skip it; main and tags build it.
            when { not { changeRequest() } }
            steps { sh 'cargo xtask build windows' }
        }
        stage('Package') {
            when { not { changeRequest() } }
            steps {
                // The Linux .deb and .rpm are deferred until release time:
                // `cargo xtask package linux` still builds them on demand.
                sh 'cargo xtask package windows'
                archiveArtifacts artifacts: 'dist/*', fingerprint: true
            }
        }
    }
}
