# main.py 

import os
from dotenv import load_dotenv
from mofa.agent_build.base.base_agent import MofaAgent, run_agent

def call_qwen_directly(user_input: str) -> str:
    """
    通过在函数内部导入 dashscope，确保 API 调用不受框架启动时的任何影响。
    """
    try:
        from dashscope import Generation
        response = Generation.call(
            model=os.getenv('LLM_MODEL', 'qwen-turbo'),
            api_key=os.getenv('LLM_API_KEY'),
            messages=[
                {"role": "system", "content": "You are a helpful AI assistant."},
                {"role": "user", "content": user_input}
            ]
        )
        # 健壮性处理
        if hasattr(response, 'status_code') and response.status_code == 200:
            output = getattr(response, 'output', None)
            if not output:
                return f"Qwen API Success, but response.output is None. Raw response: {response}"
            # 优先返回 text 字段
            text = getattr(output, 'text', None)
            if text:
                return text
            # 兼容 choices/message/content 结构
            choices = getattr(output, 'choices', None)
            if choices and isinstance(choices, list) and len(choices) > 0:
                message = getattr(choices[0], 'message', None)
                if message and hasattr(message, 'content'):
                    return message.content
            return f"Qwen API Success, but no valid text or choices in response. Raw response: {output}"
        else:
            code = getattr(response, 'code', 'Unknown')
            message = getattr(response, 'message', str(response))
            status_code = getattr(response, 'status_code', 'Unknown')
            return f"Qwen API Error: Status Code {status_code}, Code: {code}, Message: {message}"
    except Exception as e:
        return f"An exception occurred in call_qwen_directly: {str(e)}"


@run_agent
def run(agent: MofaAgent):
    try:
        load_dotenv('.env.secret')
        user_input = agent.receive_parameter('query')
        event = getattr(agent, 'event', None)
        # 只在有有效输入时才写日志，否则直接 return
        if user_input is None or str(user_input).strip() == "":
            # 可选：只在首次收到 None 时写一次日志，后续静默
            return
        agent.write_log(message=f"[DEBUG] Raw event: {event}")
        agent.write_log(message=f"Received input: {user_input}")
        agent.write_log(message="Handing over to isolated Qwen function...")
        llm_result = call_qwen_directly(user_input)
        agent.write_log(message=f"Received result from isolated function: {llm_result}")
        agent.send_output(
            agent_output_name='llm_result',
            agent_result=llm_result
        )
        return
    except Exception as e:
        error_message = f"An exception occurred in agent run loop: {str(e)}"
        agent.write_log(message=error_message, level='ERROR')
        agent.send_output(
            agent_output_name='llm_result',
            agent_result=error_message
        )
        return

def main():
    agent = MofaAgent(agent_name='qwen_agent')
    run(agent=agent)

if __name__ == "__main__":
    main()
