use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        super::version::version,
        super::auth::login,
        super::auth::callback,
        super::auth::current_session,
        super::auth::logout
    ),
    components(schemas(
        super::ErrorBody,
        super::auth::response::SessionBody,
        super::auth::response::LogoutBody,
        crate::auth::session::User
    ))
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn all_documented_body_references_resolve() {
        fn check(value: &Value, document: &Value) {
            match value {
                Value::Object(fields) => {
                    if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
                        assert!(
                            document
                                .pointer(reference.strip_prefix('#').unwrap())
                                .is_some(),
                            "unresolved {reference}"
                        );
                    }
                    for value in fields.values() {
                        check(value, document);
                    }
                }
                Value::Array(values) => {
                    for value in values {
                        check(value, document);
                    }
                }
                _ => {}
            }
        }
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        check(&document, &document);
    }
}
