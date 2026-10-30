use graphql_client::{GraphQLQuery, Response};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

macro_rules! operation {
    ($name:ident) => {
        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "graphql/schema.json",
            query_path = "graphql/operations.graphql",
            variables_derives = "Deserialize",
            response_derives = "Serialize"
        )]
        struct $name;
    };
}
operation!(Workspaces);
operation!(ProjectDetail);
operation!(ServiceInstance);
operation!(ServiceProject);
operation!(EnvironmentProject);
operation!(SshPublicKeys);
operation!(CreateSshPublicKey);

fn build<Q: GraphQLQuery>(variables: &str) -> Result<Value, String>
where
    Q::Variables: DeserializeOwned,
{
    let variables =
        serde_json::from_str(variables).map_err(|_| "Invalid Railway query variables.")?;
    serde_json::to_value(Q::build_query(variables))
        .map_err(|_| "Could not encode Railway query.".into())
}

fn build_without_variables<Q: GraphQLQuery>(
    body: &str,
    variables: Q::Variables,
) -> Result<Value, String> {
    let supplied: Value =
        serde_json::from_str(body).map_err(|_| "Invalid Railway query variables.")?;
    if supplied != json!({}) {
        return Err("This Railway query does not accept variables.".into());
    }
    // graphql_client generates a unit struct for these operations. Construct it
    // directly instead of asking Serde to decode an empty variable object into it.
    serde_json::to_value(Q::build_query(variables))
        .map_err(|_| "Could not encode Railway query.".into())
}

fn decode<Q: GraphQLQuery>(body: &str) -> Result<Value, String>
where
    Q::ResponseData: Serialize,
{
    let response: Response<Q::ResponseData> =
        serde_json::from_str(body).map_err(|_| "Could not decode Railway response.")?;
    if let Some(errors) = response.errors.filter(|errors| !errors.is_empty()) {
        return Err(errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let data = response.data.ok_or("Railway returned no data.")?;
    serde_json::to_value(data).map_err(|_| "Could not encode Railway response.".into())
}

pub(crate) fn process(operation: &str, body: &str, response: bool) -> Result<Value, String> {
    macro_rules! handle {
        ($query:ty, $variables:expr) => {
            if response {
                decode::<$query>(body)
            } else {
                build_without_variables::<$query>(body, $variables)
            }
        };
        ($query:ty) => {
            if response {
                decode::<$query>(body)
            } else {
                build::<$query>(body)
            }
        };
    }
    match operation {
        "Workspaces" => handle!(Workspaces, workspaces::Variables),
        "ProjectDetail" => handle!(ProjectDetail),
        "ServiceInstance" => handle!(ServiceInstance),
        "ServiceProject" => handle!(ServiceProject),
        "EnvironmentProject" => handle!(EnvironmentProject),
        "SshPublicKeys" => handle!(SshPublicKeys, ssh_public_keys::Variables),
        "CreateSshPublicKey" => handle!(CreateSshPublicKey),
        _ => Err("Unknown Railway operation.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::process;
    use serde_json::json;

    #[test]
    fn variable_free_queries_accept_empty_objects() {
        for operation in ["Workspaces", "SshPublicKeys"] {
            let request = process(operation, "{}", false).unwrap();
            assert_eq!(request["operationName"], operation);
        }
    }

    #[test]
    fn typed_request_preserves_operation_and_variables() {
        let request = process(
            "ServiceInstance",
            r#"{"environmentId":"environment", "serviceId":"service"}"#,
            false,
        )
        .unwrap();
        assert_eq!(request["operationName"], "ServiceInstance");
        assert_eq!(
            request["variables"],
            json!({"environmentId":"environment", "serviceId":"service"})
        );
        assert!(process("ServiceInstance", r#"{"environmentId":10}"#, false).is_err());
    }
    #[test]
    fn uuid_lookups_have_typed_project_responses() {
        for (operation, field) in [
            ("ServiceProject", "service"),
            ("EnvironmentProject", "environment"),
        ] {
            let request = process(operation, r#"{"id":"uuid"}"#, false).unwrap();
            assert_eq!(request["variables"], json!({"id":"uuid"}));
            assert!(process(operation, "{}", false).is_err());
            let response = json!({"data":{field:{"projectId":"project"}}});
            assert_eq!(
                process(operation, &response.to_string(), true).unwrap()[field]["projectId"],
                "project"
            );
        }
    }

    #[test]
    fn typed_responses_reject_errors_and_malformed_data() {
        assert_eq!(
            process("Workspaces", r#"{"data":{"externalWorkspaces":[]}}"#, true).unwrap(),
            json!({"externalWorkspaces":[]})
        );
        assert_eq!(
            process(
                "Workspaces",
                r#"{"data":{"externalWorkspaces":[]}, "errors":[{"message":"Access denied"}]}"#,
                true
            )
            .unwrap_err(),
            "Access denied"
        );
        assert!(
            process(
                "ServiceInstance",
                r#"{"data":{"serviceInstance":{"id":123}}}"#,
                true
            )
            .is_err()
        );
    }
}
