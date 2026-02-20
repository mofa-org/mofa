use mofa_sdk::llm::{FunctionDefinition, Tool};
use serde_json::json;

pub fn create_calculator_tool() -> Tool {
    Tool::function(
        "calculator",
        "Perform arithmetic calculations",
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The arithmetic expression to calculate, e.g., 2 + 3 * 4"
                }
            },
            "required": ["expression"]
        }),
    )
}

pub fn create_weather_tool() -> Tool {
    Tool::function(
        "weather_query",
        "Query weather information for a city",
        json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "The name of the city to query"
                }
            },
            "required": ["city"]
        }),
    )
}

pub fn create_news_tool() -> Tool {
    Tool::function(
        "news_query",
        "Query latest news events",
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "The topic of the news to query"
                }
            },
            "required": ["topic"]
        }),
    )
}

pub fn create_stock_tool() -> Tool {
    Tool::function(
        "stock_query",
        "Query stock market information",
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "The stock symbol to query"
                }
            },
            "required": ["symbol"]
        }),
    )
}

