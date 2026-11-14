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
            steps { sh 'cargo xtask build windows' }
        }
        stage('Package') {
            steps {
                // The Linux .deb and .rpm are deferred until release time:
                // `cargo xtask package linux` still builds them on demand.
                sh 'cargo xtask package windows'
                archiveArtifacts artifacts: 'dist/*', fingerprint: true
            }
        }
    }
}
