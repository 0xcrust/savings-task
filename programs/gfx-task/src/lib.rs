use anchor_lang::prelude::*;

declare_id!("5ebgqSXrqAAG2oofweypArHP5hqgaDh9EnSTau8GtD7g");

#[program]
pub mod gfx_task {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
