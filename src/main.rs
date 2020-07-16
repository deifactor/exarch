use anyhow::Result;
use async_std::task;
use structopt::StructOpt;

mod markgem;
mod serve;
#[derive(Debug, StructOpt)]
#[structopt(name = "exarch", about = "A static site generator for Gemini")]
enum Opt {
    /// Serve an existing tree of Markdown files.
    Serve(serve::ServeOpt),
}

fn main() -> Result<()> {
    env_logger::builder().format_module_path(true).init();
    let opt = Opt::from_args();
    match opt {
        Opt::Serve(serve_opt) => task::block_on(serve::serve(serve_opt)),
    }
}
