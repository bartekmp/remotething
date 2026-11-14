// M0 spike pipeline: every stage is one `cargo xtask` call inside ci/Dockerfile.
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
        stage('Lint') {
            steps { dir('spikes') { sh 'cargo xtask lint' } }
        }
        stage('Unit tests') {
            steps { dir('spikes') { sh 'cargo xtask test' } }
        }
        stage('Build Linux') {
            steps { dir('spikes') { sh 'cargo xtask build linux' } }
        }
        stage('Build Windows') {
            steps { dir('spikes') { sh 'cargo xtask build windows' } }
        }
    }
}
