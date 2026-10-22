use members_admin_dashboard::http::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", ApiDoc::openapi().to_pretty_json()?);
    Ok(())
}
