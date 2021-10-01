use super::{
    execute::{Executor, ExecutorError},
    template::{TemplateField, TemplateFieldFormat, TemplateFields},
};

#[cfg(not(target_family = "wasm"))]
use crate::scripting;
use async_trait::async_trait;
#[cfg(not(target_family = "wasm"))]
use ergo_database::PostgresPool;

#[cfg(not(target_family = "wasm"))]
use ergo_js::ConsoleMessage;
use futures::future::{ready, TryFutureExt};
use fxhash::FxHashMap;
#[cfg(not(target_family = "wasm"))]
use tracing::{event, instrument, Level};
use url::Url;

const FIELD_NAME: TemplateField = TemplateField::from_static(
    "name",
    TemplateFieldFormat::String,
    true,
    "The name of the action",
);
const FIELD_SCRIPT: TemplateField = TemplateField::from_static(
    "script",
    TemplateFieldFormat::String,
    false,
    "The script to execute",
);
const FIELD_ARGS: TemplateField = TemplateField::from_static(
    "args",
    TemplateFieldFormat::Object,
    true,
    "Arguments to the script. Exposed as 'args' in the script",
);

#[derive(Debug)]
pub struct JsExecutor {
    template_fields: TemplateFields,
}

impl JsExecutor {
    pub fn new() -> JsExecutor {
        let template_fields = vec![FIELD_NAME, FIELD_SCRIPT, FIELD_ARGS].into();

        JsExecutor { template_fields }
    }
}

#[async_trait]
impl Executor for JsExecutor {
    fn name(&self) -> &'static str {
        "js"
    }

    #[cfg(not(target_family = "wasm"))]
    async fn execute(
        &self,
        _pg_pool: PostgresPool,
        payload: FxHashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutorError> {
        let (console, result) = scripting::POOL
            .run(move || async move {
                let name = FIELD_NAME.extract_str(&payload)?.unwrap_or("script");
                let script = FIELD_SCRIPT.extract_str(&payload)?.unwrap_or("");

                let mut runtime = scripting::create_executor_runtime();
                if let Some(args) = FIELD_ARGS.extract_object(&payload)? {
                    runtime.set_global_value("args", args).map_err(|e| {
                        ExecutorError::CommandError {
                            source: e,
                            result: serde_json::Value::Null,
                        }
                    })?;
                } else {
                    runtime
                        .set_global_value("args", &serde_json::json!({}))
                        .map_err(|e| ExecutorError::CommandError {
                            source: e,
                            result: serde_json::Value::Null,
                        })?;
                }

                event!(Level::DEBUG, %script, "executing script");
                let name_url =
                    Url::parse(&format!("https://ergo/executor/{}", name)).map_err(|_| {
                        ExecutorError::FieldFormatError {
                            field: "name".to_string(),
                            subfield: None,
                            expected: "A formattable name".to_string(),
                        }
                    })?;
                let run_result = runtime.run_main_module(name_url, script.to_string()).await;
                let mut console = serde_json::to_value(runtime.take_console_messages())
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new()));

                run_result.map_err(|e| ExecutorError::CommandError {
                    source: e,
                    result: std::mem::take(&mut console),
                })?;

                let result = runtime
                    .get_global_value::<serde_json::Value>("result")
                    .map_err(|e| ExecutorError::CommandError {
                        source: e.into(),
                        result: std::mem::take(&mut console),
                    })?
                    .unwrap_or(serde_json::Value::Null);
                Ok::<_, ExecutorError>((console, result))
            })
            .await?;

        Ok(serde_json::json!({
            "result": result,
            "console": console
        }))
    }

    fn template_fields(&self) -> &TemplateFields {
        &self.template_fields
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn runs_script() {
        let script = r##"
            
        "##;
    }

    #[tokio::test]
    #[ignore]
    async fn runs_async_script() {}

    #[tokio::test]
    #[ignore]
    async fn script_exception() {}

    #[tokio::test]
    #[ignore]
    async fn async_script_exception() {}
}
