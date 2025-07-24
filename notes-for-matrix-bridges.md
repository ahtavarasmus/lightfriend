These are notes currently for running the matrix bridges I have made.



### database config:

*if postgres(good for multiple users)*:
create new bridge db
```createdb mv_<service_name>```
login to postgres
```psql -d mw_<service_name>```
create new user to this bridge db and grant permissions
```CREATE USER mw_<service_name> WITH PASSWORD 'password-for-the-matrix-bridge';```
```GRANT ALL PRIVILEGES ON DATABASE mw_<service_name> TO mw_<service_name>;```
```GRANT ALL ON SCHEMA public TO mw_telegram;```
if db already exists
```dropdb mw_<service_name>```
endif

*if sqlite(enough for single user)*:
endif

### configuration files

*in double_puppet.yaml*:
    id: doublepuppet
    url: null
    as_token: <as_token here> 
    hs_token: anotherrandomstring1234567890-doesn't matter what this is just some random
    sender_localpart: yetanotherrandomstring0987654321-doens't matter just some random
    rate_limited: false
    namespaces:
      users:
      - regex: '@.*:localhost'
        exclusive: false
    """
endif

*in config.yaml*:
    database: postgres://mw_<service_name>:password-for-the-mautrix-bridge@localhost/mw_<service_name>?sslmode=disable

    homeserver->address->http://localhost:8008
    homeserver->domain->localhost

    permissions:
        "*": user
        "@adminuser:localhost": admin

    (in whatsapp bridges:
        backfill-> enabled: false

        history_sync-> max_initial_conversations: 0

        if multiple users:
            async_transactions: true
            endif

        for double_puppeting:
        double_puppet->secrets:
            localhost: as_token:<as_token_from_double_puppet.yaml>:
    )
    (in telegram bridges:
        telegram-> api_id: <id>
        telegram-> api_hash: <hash>

        for double_puppeting:
        bridge->login_shared_secret_map:
            localhost: as_token:<as_token_from_double_puppet.yaml>
    )
endin

*in homeserver.yaml*:
    server_name: "localhost"
    pid_file: "/Users/YOUR_USERNAME/matrix-dev/homeserver.pid"
    listeners:
      - port: 8008
        tls: false
        type: http
        x_forwarded: false
        bind_addresses: ['127.0.0.1']
        resources:
          - names: [client, federation]
            compress: false
    database:
      name: sqlite3
      args:
        database: /Users/YOUR_USERNAME/matrix-dev/homeserver.db
    enable_registration: true
    registration_shared_secret: "generate this with openssl rand -base64 32"

    Replace YOUR_USERNAME with your macOS username (find it with whoami). Key changes:

        server_name: Set to localhost.
        listeners: Binds to 127.0.0.1:8008 for HTTP (no TLS for dev).
        database: Uses SQLite with a local database file.
        enable_registration: Allows user registration for testing.
        registration_shared_secret: Set a secret for admin registration (e.g., your-secret-here). Your code uses this for registering users.

    app_service_config_files:
      - /Users/YOUR_USERNAME/matrix-dev/mautrix-<service_name>-registration.yaml
      - /Users/YOUR_USERNAME/matrix-dev/mautrix-<service_name>-registration.yaml
      - /Users/YOUR_USERNAME/matrix-dev/doublepuppet.yaml

endin

