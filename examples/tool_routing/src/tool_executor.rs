use mofa_sdk::llm::{Tool, ToolExecutor};
use std::sync::Arc;

/// 示例工具执行器
/// Example tool executor
pub struct ExampleToolExecutor;

#[async_trait::async_trait]
impl ToolExecutor for ExampleToolExecutor {
    async fn execute(&self, name: &str, arguments: &str) -> mofa_sdk::llm::LLMResult<String> {
        println!("Executing tool: {} with args: {}", name, arguments);

        match name {
            "calculator" => {
                let result = self.execute_calculator(arguments).await;
                Ok(result)
            }
            "weather_query" => {
                let result = self.execute_weather(arguments).await;
                Ok(result)
            }
            "news_query" => {
                let result = self.execute_news(arguments).await;
                Ok(result)
            }
            "stock_query" => {
                let result = self.execute_stock(arguments).await;
                Ok(result)
            }
            _ => {
                Err(mofa_sdk::llm::LLMError::Other(format!("Unknown tool: {}", name)))
            }
        }
    }

    async fn available_tools(&self) -> mofa_sdk::llm::LLMResult<Vec<Tool>> {
        Ok(Vec::new())
    }
}

impl ExampleToolExecutor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    async fn execute_calculator(&self, arguments: &str) -> String {
        // 简单的计算器实现，仅用于演示
        // Simple calculator implementation, for demonstration only
        #[derive(serde::Deserialize)]
        struct CalculatorArgs {
            expression: String,
        }

        if let Ok(args) = serde_json::from_str::<CalculatorArgs>(arguments) {
            let expression = args.expression.replace(" ", "");

            // 非常简单的计算逻辑，仅支持整数和+-*/
            // Very simple calculation logic, only supports integers and +-*/
            if let Ok(result) = self.simple_eval(&expression) {
                format!("Calculation result: {}", result)
            } else {
                "Calculation error: Unsupported expression".to_string()
            }
        } else {
            "Calculation error: Invalid arguments".to_string()
        }
    }

    async fn execute_weather(&self, arguments: &str) -> String {
        #[derive(serde::Deserialize)]
        struct WeatherArgs {
            city: String,
        }

        if let Ok(args) = serde_json::from_str::<WeatherArgs>(arguments) {
            format!("Weather in {}: Sunny, 25°C", args.city)
        } else {
            "Weather query error: Invalid arguments".to_string()
        }
    }

    async fn execute_news(&self, arguments: &str) -> String {
        #[derive(serde::Deserialize)]
        struct NewsArgs {
            topic: String,
        }

        if let Ok(args) = serde_json::from_str::<NewsArgs>(arguments) {
            format!("Latest news about {}: Rust 1.75 released with new features", args.topic)
        } else {
            "News query error: Invalid arguments".to_string()
        }
    }

    async fn execute_stock(&self, arguments: &str) -> String {
        #[derive(serde::Deserialize)]
        struct StockArgs {
            symbol: String,
        }

        if let Ok(args) = serde_json::from_str::<StockArgs>(arguments) {
            format!("Stock {}: Price $100.50, Change +2.5%", args.symbol)
        } else {
            "Stock query error: Invalid arguments".to_string()
        }
    }

    fn simple_eval(&self, expr: &str) -> Result<i64, ()> {
        // 仅用于演示，实际项目应使用成熟的计算库
        // For demonstration only, actual projects should use mature math libraries
        use std::str::FromStr;

        let mut chars = expr.chars().peekable();
        let mut num_str = String::new();
        let mut result: i64 = 0;
        let mut current_op = '+';

        while let Some(c) = chars.next() {
            if c.is_digit(10) {
                num_str.push(c);
            } else {
                let num = i64::from_str(&num_str).map_err(|_| ())?;

                match current_op {
                    '+' => result += num,
                    '-' => result -= num,
                    '*' => result *= num,
                    '/' => result /= num,
                    _ => return Err(()),
                }

                current_op = c;
                num_str.clear();
            }
        }

        let num = i64::from_str(&num_str).map_err(|_| ())?;
        match current_op {
            '+' => Ok(result + num),
            '-' => Ok(result - num),
            '*' => Ok(result * num),
            '/' => Ok(result / num),
            _ => Err(()),
        }
    }
}
