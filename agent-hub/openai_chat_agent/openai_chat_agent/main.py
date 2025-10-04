import os
from dotenv import load_dotenv
from mofa.agent_build.base.base_agent import MofaAgent, run_agent


def call_openai_directly(user_input: str) -> str:
    import openai
    client = openai.OpenAI(
        api_key=os.getenv('LLM_API_KEY'),
        base_url=os.getenv('LLM_API_BASE')
    )

    response = client.chat.completions.create(
        model=os.getenv('LLM_MODEL', 'gpt-4o'),
        messages=[
            {"role": "system", "content": "You are a helpful AI assistant."},
            {"role": "user", "content": user_input}
        ],
        stream=False
    )
    return response.choices[0].message.content


@run_agent
def run(agent: MofaAgent):
    try:
        load_dotenv('.env.secret')
        user_input = agent.receive_parameter('query')
        agent.write_log(message=f"Received input: {user_input}")

        agent.write_log(message="Handing over to isolated OpenAI function...")
        llm_result = call_openai_directly(user_input)
        agent.write_log(message=f"Received result from isolated function: {llm_result}")
        
        agent.send_output(
            agent_output_name='llm_result',
            agent_result=llm_result
        )
    except Exception as e:
        error_message = f"An exception occurred: {str(e)}"
        
        # 使用 MofaAgent 正确的日志记录方法
        agent.write_log(message=error_message, level='ERROR')
        
        # 同样将这个错误信息发送出去
        agent.send_output(
            agent_output_name='llm_result',
            agent_result=error_message
        )

def main():
    agent = MofaAgent(agent_name='openai_chat_agent')
    run(agent=agent)

if __name__ == "__main__":
    main()
