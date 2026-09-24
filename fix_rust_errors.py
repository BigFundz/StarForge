import os
import re

def replace_in_file(path, old, new):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        c = f.read()
    if old in c:
        c = c.replace(old, new)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(c)

def regex_replace_in_file(path, pattern, repl):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        c = f.read()
    new_c = re.sub(pattern, repl, c)
    if c != new_c:
        with open(path, 'w', encoding='utf-8') as f:
            f.write(new_c)

# 1. deploy.rs
replace_in_file('src/commands/deploy.rs',
    '        return run_dry_run(\n            &wasm_path,\n            &wasm_bytes,\n            &wasm_hash,\n            wasm_size_kb,\n            wallet,\n            &args.network,\n        );',
    '        return run_dry_run(\n            &wasm_path,\n            &wasm_bytes,\n            &wasm_hash,\n            wasm_size_kb,\n            wallet,\n            &args.network,\n        ).await;')

# 2. help.rs
replace_in_file('src/commands/help.rs', 'return handle_settings(&args);', 'return handle_settings(&args).await;')
replace_in_file('src/commands/help.rs', 
'''        p::kv(
            "Enabled categories",
            if enabled.is_empty() {
                "(all)"
            } else {
                enabled.join(", ").as_str()
            },
        );''',
'''        let en_str = enabled.join(", ");
        p::kv(
            "Enabled categories",
            if enabled.is_empty() {
                "(all)"
            } else {
                &en_str
            },
        );''')
replace_in_file('src/commands/help.rs', 
'''        p::kv(
            "Disabled categories",
            if disabled.is_empty() {
                "(none)"
            } else {
                disabled.join(", ").as_str()
            },
        );''',
'''        let dis_str = disabled.join(", ");
        p::kv(
            "Disabled categories",
            if disabled.is_empty() {
                "(none)"
            } else {
                &dis_str
            },
        );''')

# 3. template.rs commands
replace_in_file('src/commands/template.rs', 'TemplateCommands::Import {', 'TemplateCommands::Install {')
replace_in_file('src/commands/template.rs', 'TemplateCommands::Info { name } => info(name),', 'TemplateCommands::Info { name } => info(name).await,')
replace_in_file('src/commands/template.rs', '.await.with_context(||', '.await.context(||')
replace_in_file('src/commands/template.rs', '.with_context(||', '.context(||')
replace_in_file('src/commands/template.rs', 'use anyhow::Result;', 'use anyhow::{Context, Result};')
replace_in_file('src/commands/template.rs', 'TemplateCommands::Fetch {\n            source,\n            name,\n            version,\n            force,\n        } => install(source, name, version, force).await', 'TemplateCommands::Fetch {\n            source,\n            name,\n            version,\n            force,\n        } => crate::utils::template::install(source, name, version, force).await')

# 4. wallet.rs
replace_in_file('src/commands/wallet.rs', 
    '        } => rotate_wallet(\n            name,\n            fund,\n            network,\n            encryption_password,\n            mem,\n            iterations,\n            parallelism,\n            backup,\n        ),',
    '        } => rotate_wallet(\n            name,\n            fund,\n            network,\n            encryption_password,\n            mem,\n            iterations,\n            parallelism,\n            backup,\n        ).await,')

# 5. ai_context.rs
replace_in_file('src/utils/ai_context.rs', 'let mut items = self.collect_context(project_path).await?;', 'let items = self.collect_context(project_path).await?;')

# 6. ai_tutorial.rs
replace_in_file('src/utils/ai_tutorial.rs', 'recommended.sort_by_key(|t| t.difficulty as i32);', 'recommended.sort_by_key(|t| t.difficulty.clone() as i32);')
replace_in_file('src/utils/ai_tutorial.rs', 'if tutorial.difficulty as i32 <= skill_level as i32 + 1 {', 'if tutorial.difficulty.clone() as i32 <= skill_level.clone() as i32 + 1 {')

# 7. security.rs
replace_in_file('src/commands/security.rs', 'let created = track_findings("audit", &tracking_findings)?;', 'let created = crate::utils::security::track_findings("audit", &tracking_findings)?;')
replace_in_file('src/commands/security.rs', 'generate_github_actions_workflow(&args.path, min_score.unwrap_or(80.0));', 'crate::utils::security::generate_github_actions_workflow(&args.path, min_score.unwrap_or(80.0));')
replace_in_file('src/commands/security.rs', 'let html = format_html_report(&result);', 'let html = crate::utils::security::format_html_report(&result);')

# 8. test.rs
replace_in_file('src/commands/test.rs', 'let mut optimizer = test_optimizer::TestOptimizer::new()?;', 'let mut optimizer = crate::utils::test_optimizer::TestOptimizer::new()?;')
replace_in_file('src/commands/test.rs', 'test_optimizer::TestCaseTiming', 'crate::utils::test_optimizer::TestCaseTiming')
replace_in_file('src/commands/test.rs', 'test_optimizer::export_optimization_report', 'crate::utils::test_optimizer::export_optimization_report')
replace_in_file('src/commands/test.rs', 'test_optimizer::render_optimization_html_report', 'crate::utils::test_optimizer::render_optimization_html_report')

# 9. config.rs
replace_in_file('src/commands/config.rs', 'let path = database::db_path();', 'let path = crate::utils::database::db_path();')
replace_in_file('src/commands/config.rs', 'database::Database::open', 'crate::utils::database::Database::open')
replace_in_file('src/commands/config.rs', 'database::migrate_from_toml', 'crate::utils::database::migrate_from_toml')
replace_in_file('src/commands/config.rs', 'database::restore_database', 'crate::utils::database::restore_database')
replace_in_file('src/commands/config.rs', 'database::export_to_toml', 'crate::utils::database::export_to_toml')

# 10. ai_test.rs
replace_in_file('src/commands/ai_test.rs', 'let contract_name = args.name.unwrap_or_else(|| {', 'let contract_name = args.name.clone().unwrap_or_else(|| {')
replace_in_file('src/commands/ai_test.rs', 'optimization_goals: goals,', 'optimization_goals: goals.clone(),')

print("Applied fixes")
