The schema is the unmodified Railway CLI schema from `src/gql/schema.json` at
commit `c650a319e72b0ea033153ba87915a7a74848028f`.

Like Railway CLI, Forwarder uses `graphql_client` 0.14.0 to generate typed query
variables and response data. Operations live in `operations.graphql` and are
checked against this schema at compile time. Rust supplies OAuth-authenticated
HTTP transport and maps the validated results into the app's catalog models.
