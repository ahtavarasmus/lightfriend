#!/bin/sh
set -e

# Fix permissions on /data directory
chown -R 991:991 /data

# Switch to synapse user (991) and run synapse
exec setpriv --reuid=991 --regid=991 --clear-groups \
    python -m synapse.app.homeserver --config-path=/config/homeserver.yaml
