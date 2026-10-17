plugin_toolkit::instrument::bootstrap!();

#[allow(unused_imports)]
use garage::tools as _;

fn main() -> plugin_toolkit::anyhow::Result<()> {
    plugin_toolkit::plugin::Plugin::named("garage")
        .version(env!("CARGO_PKG_VERSION"))
        .tools(["garage."])
        .service(garage::GarageBackend::new("garage"))
        .serve()
}
