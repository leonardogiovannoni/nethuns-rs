mod af_xdp;
mod api;
mod netmap;
mod strategy;
mod meter;
mod forward_mt;
mod forward;
use anyhow::Result;



fn main() -> Result<()> {
    forward_mt::routine()
}