流失对话使用示例
```asm
use futures::StreamExt;
use mofa_sdk::llm::{LLMAgentBuilder, openai_from_env};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
let agent = LLMAgentBuilder::new("my-agent")
.with_provider(Arc::new(openai_from_env()))
.with_system_prompt("You are a helpful assistant.")
.build();

      // 流式问答
      let mut stream = agent.ask_stream("Tell me a story").await?;
      while let Some(result) = stream.next().await {
          match result {
              Ok(text) => print!("{}", text),
              Err(e) => einfo!("Error: {}", e),
          }
      }
      info!();

      // 流式多轮对话
      let mut stream = agent.chat_stream("Hello!").await?;
      while let Some(result) = stream.next().await {
          if let Ok(text) = result {
              print!("{}", text);
          }
      }
      info!();

      // 流式对话并获取完整响应
      let (mut stream, full_rx) = agent.chat_stream_with_full("What's 2+2?").await?;
      while let Some(result) = stream.next().await {
          if let Ok(text) = result {
              print!("{}", text);
          }
      }
      let full_response = full_rx.await?;
      info!("\nFull: {}", full_response);

      Ok(())
}
```
