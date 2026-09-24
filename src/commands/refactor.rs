use crate::utils::ai_refactor::RefactorCommands;
use anyhow::Result;

pub async fn handle(cmd: RefactorCommands) -> Result<()> {
    crate::utils::ai_refactor::handle(cmd).await
}
