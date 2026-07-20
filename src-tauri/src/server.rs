// Besure AI Context — Desktop App Embedded Server
// 直接复用 besure_lib 的 REST API，不维护两份

use besure_lib::ai::rest_api::ApiServer;

pub async fn start_server(port: u16) -> anyhow::Result<()> {
    ApiServer::new(port).run().await
}
