use crate::utils::{config, debugger, debugger::Debugger, print as p, soroban};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use dialoguer::{Input, Select};
use std::sync::Mutex;

static DEBUGGER: once_cell::sync::Lazy<Mutex<Option<Debugger>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

#[derive(Subcommand)]
pub enum DebugCommands {
    /// Start a new debugging session for a contract
    Start(StartArgs),
    /// Manage breakpoints (add, list, remove, enable, disable)
    #[command(subcommand)]
    Breakpoint(BreakpointCommands),
    /// Step through execution (into, over, out)
    Step(StepArgs),
    /// Continue execution until next breakpoint
    Continue,
    /// Inspect contract variables
    Inspect(InspectArgs),
    /// Display the current call stack
    Stack,
    /// Launch the interactive debugging interface
    Ui(UiArgs),
}

#[derive(Args)]
pub struct StartArgs {
    /// Contract ID to debug (for deployed contracts)
    #[arg(long)]
    pub contract_id: Option<String>,
    /// Path to compiled WASM (for local debugging)
    #[arg(long)]
    pub wasm: Option<String>,
    /// Network to use
    #[arg(long, default_value = "testnet")]
    pub network: String,
}

#[derive(Subcommand)]
pub enum BreakpointCommands {
    /// Add a new breakpoint at a function entry
    Add(BreakpointAddArgs),
    /// List all breakpoints
    List,
    /// Remove a breakpoint by ID
    Remove(BreakpointRemoveArgs),
    /// Enable a breakpoint by ID
    Enable(BreakpointEnableArgs),
    /// Disable a breakpoint by ID
    Disable(BreakpointDisableArgs),
}

#[derive(Args)]
pub struct BreakpointAddArgs {
    /// Function name to break on
    pub function: String,
    /// Contract ID (optional, applies to all contracts if omitted)
    #[arg(long)]
    pub contract_id: Option<String>,
    /// Conditional breakpoint expression
    #[arg(long)]
    pub condition: Option<String>,
}

#[derive(Args)]
pub struct BreakpointRemoveArgs {
    /// Breakpoint ID to remove
    pub id: usize,
}

#[derive(Args)]
pub struct BreakpointEnableArgs {
    /// Breakpoint ID to enable
    pub id: usize,
}

#[derive(Args)]
pub struct BreakpointDisableArgs {
    /// Breakpoint ID to disable
    pub id: usize,
}

#[derive(Args)]
pub struct StepArgs {
    /// Step direction: into, over, or out
    #[arg(default_value = "into")]
    pub direction: String,
}

#[derive(Args)]
pub struct InspectArgs {
    /// Variable name to inspect (omit to list all)
    pub name: Option<String>,
    /// Search pattern for variable names/values
    #[arg(long)]
    pub search: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct UiArgs {
    /// Contract ID to debug
    #[arg(long)]
    pub contract_id: Option<String>,
    /// Path to compiled WASM
    #[arg(long)]
    pub wasm: Option<String>,
    /// Network to use
    #[arg(long, default_value = "testnet")]
    pub network: String,
}

pub async fn handle(cmd: DebugCommands) -> Result<()> {
    match cmd {
        DebugCommands::Start(args) => handle_start(args).await,
        DebugCommands::Breakpoint(cmd) => handle_breakpoint(cmd).await,
        DebugCommands::Step(args) => handle_step(args).await,
        DebugCommands::Continue => handle_continue().await,
        DebugCommands::Inspect(args) => handle_inspect(args).await,
        DebugCommands::Stack => handle_stack().await,
        DebugCommands::Ui(args) => handle_ui(args).await,
    }
}

async fn handle_start(args: StartArgs) -> Result<()> {
    p::header("Start Debugging Session");
    p::separator();

    if let Some(ref cid) = args.contract_id {
        config::validate_contract_id(cid)?;
        p::kv("Contract ID", cid);
    }
    if let Some(ref wasm) = args.wasm {
        p::kv("WASM Path", wasm);
    }
    p::kv("Network", &args.network);
    p::separator();

    let mut debugger = Debugger::new(args.contract_id.clone(), args.wasm.clone(), &args.network);

    if let Some(ref cid) = args.contract_id {
        p::step(1, 1, "Fetching contract state from Soroban RPC…");
        match soroban::inspect_contract(cid, &args.network).await {
            Ok(inspect) => {
                let sess = &mut debugger.session;
                let mut vars = Vec::new();
                for entry in &inspect.instance_storage {
                    vars.push(debugger::Variable {
                        name: entry.key.clone(),
                        var_type: "storage".to_string(),
                        value: entry.value.clone(),
                    });
                }
                sess.set_variables(vars);

                let frames = vec![debugger::StackFrame {
                    function: "contract_init".to_string(),
                    contract_id: Some(cid.clone()),
                    source_location: None,
                    variables: inspect
                        .instance_storage
                        .iter()
                        .map(|e| debugger::Variable {
                            name: e.key.clone(),
                            var_type: "storage".to_string(),
                            value: e.value.clone(),
                        })
                        .collect(),
                }];
                sess.set_call_stack(frames);
                sess.add_step_history("Debug session started".to_string());
            }
            Err(e) => {
                p::warn(&format!("Could not fetch contract state: {}", e));
                p::info("Starting session with empty state. Use `inspect` after invoking.");
            }
        }
    }

    let mut guard = DEBUGGER.lock().unwrap();
    *guard = Some(debugger);

    p::success("Debugging session started.");
    p::info("Use `starforge debug breakpoint add <function>` to set breakpoints.");
    p::info("Use `starforge debug inspect` to inspect variables.");
    p::info("Use `starforge debug ui` to launch the interactive debugger.");
    p::separator();
    Ok(())
}

async fn handle_breakpoint(cmd: BreakpointCommands) -> Result<()> {
    let mut guard = DEBUGGER.lock().unwrap();
    let debugger = guard.as_mut().ok_or_else(|| {
        anyhow::anyhow!("No active debugging session. Start one with `starforge debug start`.")
    })?;

    match cmd {
        BreakpointCommands::Add(args) => {
            let bp = debugger.add_breakpoint(args.contract_id, &args.function, args.condition);
            p::success(&format!(
                "Breakpoint {} set on function '{}'",
                bp.id.to_string().cyan().bold(),
                bp.function.cyan()
            ));
        }
        BreakpointCommands::List => {
            let bps = debugger.list_breakpoints();
            if bps.is_empty() {
                p::info("No breakpoints set.");
            } else {
                p::header("Breakpoints");
                for bp in bps {
                    let status = if bp.enabled {
                        "enabled".green()
                    } else {
                        "disabled".dimmed()
                    };
                    let cid = bp
                        .contract_id
                        .as_deref()
                        .unwrap_or("(any contract)")
                        .to_string();
                    println!(
                        "  {}  {} @ {} (contract: {}) [{}] hits: {}",
                        format!("#{}", bp.id).cyan().bold(),
                        bp.function.bright_white(),
                        cid.dimmed(),
                        cid,
                        status,
                        bp.hit_count.to_string().yellow()
                    );
                }
            }
        }
        BreakpointCommands::Remove(args) => {
            if debugger.remove_breakpoint(args.id) {
                p::success(&format!("Breakpoint {} removed.", args.id));
            } else {
                anyhow::bail!("Breakpoint {} not found.", args.id);
            }
        }
        BreakpointCommands::Enable(args) => {
            if debugger.enable_breakpoint(args.id) {
                p::success(&format!("Breakpoint {} enabled.", args.id));
            } else {
                anyhow::bail!("Breakpoint {} not found.", args.id);
            }
        }
        BreakpointCommands::Disable(args) => {
            if debugger.disable_breakpoint(args.id) {
                p::success(&format!("Breakpoint {} disabled.", args.id));
            } else {
                anyhow::bail!("Breakpoint {} not found.", args.id);
            }
        }
    }
    Ok(())
}

async fn handle_step(args: StepArgs) -> Result<()> {
    let mut guard = DEBUGGER.lock().unwrap();
    let debugger = guard.as_mut().ok_or_else(|| {
        anyhow::anyhow!("No active debugging session. Start one with `starforge debug start`.")
    })?;

    match args.direction.as_str() {
        "into" => {
            debugger.step_into();
            p::success("Stepping into next function call…");
        }
        "over" => {
            debugger.step_over();
            p::success("Stepping over next function call…");
        }
        "out" => {
            debugger.step_out();
            p::success("Stepping out of current function…");
        }
        _ => anyhow::bail!(
            "Invalid step direction '{}'. Use 'into', 'over', or 'out'.",
            args.direction
        ),
    }

    debugger
        .session
        .add_step_history(format!("step {}", args.direction));

    simulate_step(debugger)?;

    Ok(())
}

fn simulate_step(debugger: &mut Debugger) -> Result<()> {
    let current_fn = debugger
        .session
        .current_function
        .clone()
        .unwrap_or_default();

    p::separator();
    p::kv_accent(
        "Current Depth",
        &debugger.session.call_stack.len().to_string(),
    );
    p::kv_accent("Step Count", &debugger.session.step_count.to_string());

    if !current_fn.is_empty() {
        p::kv_accent("Function", &current_fn);
    }

    let vars = debugger.inspect_all_variables();
    if !vars.is_empty() {
        println!();
        p::info("Variables in scope:");
        for v in vars.iter().take(10) {
            println!(
                "  {} {} = {}",
                format!("{}:", v.name).dimmed(),
                format!("({})", v.var_type).dimmed(),
                v.value.bright_white()
            );
        }
        if vars.len() > 10 {
            p::info(&format!("… and {} more variables", vars.len() - 10));
        }
    }

    let frames = debugger.inspect_call_stack();
    if !frames.is_empty() {
        println!();
        p::info("Call stack:");
        for (i, frame) in frames.iter().enumerate() {
            let marker = if i == frames.len() - 1 {
                "→".cyan().to_string()
            } else {
                " ".to_string()
            };
            println!(
                "  {} {} {}",
                marker,
                frame.function.bright_white(),
                frame
                    .contract_id
                    .as_ref()
                    .map(|cid| format!("({})", &cid[..8]).dimmed().to_string())
                    .unwrap_or_default()
            );
        }
    }
    p::separator();

    Ok(())
}

async fn handle_continue() -> Result<()> {
    let mut guard = DEBUGGER.lock().unwrap();
    let debugger = guard.as_mut().ok_or_else(|| {
        anyhow::anyhow!("No active debugging session. Start one with `starforge debug start`.")
    })?;

    debugger.continue_execution();
    debugger.session.add_step_history("continue".to_string());
    p::success("Execution resumed.");
    Ok(())
}

async fn handle_inspect(args: InspectArgs) -> Result<()> {
    let guard = DEBUGGER.lock().unwrap();
    let debugger = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active debugging session. Start one with `starforge debug start`.")
    })?;

    if args.json {
        if let Some(ref name) = args.name {
            let var = debugger.find_variable(name);
            println!("{}", serde_json::to_string_pretty(&var)?);
        } else {
            let vars = debugger.inspect_all_variables();
            println!("{}", serde_json::to_string_pretty(&vars)?);
        }
        return Ok(());
    }

    if let Some(ref search) = args.search {
        let results = debugger.search_variables(search);
        if results.is_empty() {
            p::info(&format!("No variables matching '{}'", search));
        } else {
            p::header(&format!("Variables matching '{}'", search));
            for v in results {
                println!(
                    "  {} {} = {}",
                    format!("{}:", v.name).dimmed(),
                    format!("({})", v.var_type).dimmed(),
                    v.value.bright_white()
                );
            }
        }
        return Ok(());
    }

    if let Some(ref name) = args.name {
        match debugger.find_variable(name) {
            Some(var) => {
                p::header(&format!("Variable: {}", var.name));
                p::kv("Name", &var.name);
                p::kv("Type", &var.var_type);
                p::kv("Value", &var.value);
            }
            None => {
                let results = debugger.search_variables(name);
                if results.is_empty() {
                    anyhow::bail!("Variable '{}' not found in current scope.", name);
                } else {
                    p::info(&format!("'{}' did not match exactly. Did you mean:", name));
                    for v in results {
                        println!("  - {} ({})", v.name.cyan(), v.var_type.dimmed());
                    }
                }
            }
        }
    } else {
        let vars = debugger.inspect_all_variables();
        if vars.is_empty() {
            p::info("No variables in scope. Invoke a contract function first.");
        } else {
            p::header(&format!("Variables ({} total)", vars.len()));
            for v in vars {
                let value_short = if v.value.len() > 60 {
                    format!("{}…", &v.value[..60])
                } else {
                    v.value.clone()
                };
                println!(
                    "  {} {} = {}",
                    format!("{}:", v.name).dimmed(),
                    format!("({})", v.var_type).dimmed(),
                    value_short.bright_white()
                );
            }
        }
    }
    Ok(())
}

async fn handle_stack() -> Result<()> {
    let guard = DEBUGGER.lock().unwrap();
    let debugger = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No active debugging session. Start one with `starforge debug start`.")
    })?;

    let frames = debugger.inspect_call_stack();
    if frames.is_empty() {
        p::info("Call stack is empty.");
    } else {
        p::header(&format!(
            "Call Stack ({} frame{})",
            frames.len(),
            if frames.len() == 1 { "" } else { "s" }
        ));
        p::separator();
        for (i, frame) in frames.iter().enumerate() {
            let depth = frames.len() - i;
            let arrow = if i == frames.len() - 1 {
                "→".cyan()
            } else {
                " ".dimmed()
            };
            println!(
                "  {} #{} {}",
                arrow,
                depth.to_string().dimmed(),
                frame.function.bright_white().bold()
            );
            if let Some(ref cid) = frame.contract_id {
                println!("    {} {}", "Contract:".dimmed(), cid.dimmed());
            }
            if let Some(ref loc) = frame.source_location {
                println!("    {} {}", "Location:".dimmed(), loc.dimmed());
            }
            if !frame.variables.is_empty() {
                println!(
                    "    {} {}",
                    "Variables:".dimmed(),
                    frame
                        .variables
                        .iter()
                        .map(|v| v.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .dimmed()
                );
            }
        }
    }
    Ok(())
}

async fn handle_ui(args: UiArgs) -> Result<()> {
    if let Some(ref cid) = args.contract_id {
        config::validate_contract_id(cid)?;
    }

    let has_session = {
        let guard = DEBUGGER.lock().unwrap();
        guard.is_some()
    };

    if !has_session {
        let start_args = StartArgs {
            contract_id: args.contract_id.clone(),
            wasm: args.wasm.clone(),
            network: args.network.clone(),
        };
        handle_start(start_args).await?;
    }

    println!();
    p::header("Interactive Debugger");
    p::separator();
    p::info("Welcome to the Soroban Contract Debugger.");
    p::separator();
    println!();

    loop {
        let selection = Select::new()
            .with_prompt("Debug Action")
            .items(&[
                "Step Into",
                "Step Over",
                "Step Out",
                "Continue",
                "Inspect Variables",
                "Inspect Call Stack",
                "Manage Breakpoints",
                "Exit Debugger",
            ])
            .default(0)
            .interact()?;

        match selection {
            0 => {
                handle_step(StepArgs {
                    direction: "into".to_string(),
                })
                .await?;
            }
            1 => {
                handle_step(StepArgs {
                    direction: "over".to_string(),
                })
                .await?;
            }
            2 => {
                handle_step(StepArgs {
                    direction: "out".to_string(),
                })
                .await?;
            }
            3 => {
                handle_continue().await?;
            }
            4 => {
                let name: String = Input::new()
                    .with_prompt("Variable name (empty to show all)")
                    .allow_empty(true)
                    .interact_text()?;
                let name = if name.is_empty() { None } else { Some(name) };
                handle_inspect(InspectArgs {
                    name,
                    search: None,
                    json: false,
                })
                .await?;
            }
            5 => {
                handle_stack().await?;
            }
            6 => {
                manage_breakpoints_interactive().await?;
            }
            7 => {
                p::success("Exiting debugger.");
                break;
            }
            _ => unreachable!(),
        }

        println!();
    }

    Ok(())
}

async fn manage_breakpoints_interactive() -> Result<()> {
    let selection = Select::new()
        .with_prompt("Breakpoint Action")
        .items(&[
            "List Breakpoints",
            "Add Breakpoint",
            "Remove Breakpoint",
            "Go Back",
        ])
        .default(0)
        .interact()?;

    match selection {
        0 => {
            handle_breakpoint(BreakpointCommands::List).await?;
        }
        1 => {
            let function: String = Input::new().with_prompt("Function name").interact_text()?;
            let contract_id: String = Input::new()
                .with_prompt("Contract ID (optional)")
                .allow_empty(true)
                .interact_text()?;
            let condition: String = Input::new()
                .with_prompt("Condition (optional)")
                .allow_empty(true)
                .interact_text()?;
            let contract_id = if contract_id.is_empty() {
                None
            } else {
                Some(contract_id)
            };
            let condition = if condition.is_empty() {
                None
            } else {
                Some(condition)
            };
            handle_breakpoint(BreakpointCommands::Add(BreakpointAddArgs {
                function,
                contract_id,
                condition,
            }))
            .await?;
        }
        2 => {
            let id: usize = Input::new()
                .with_prompt("Breakpoint ID to remove")
                .interact_text()?;
            handle_breakpoint(BreakpointCommands::Remove(BreakpointRemoveArgs { id })).await?;
        }
        3 => {}
        _ => unreachable!(),
    }
    Ok(())
}
