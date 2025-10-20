import os

from mofa.agent_build.base.base_agent import MofaAgent, run_agent
from .call_sth import choice_and_run_llm_model

@run_agent
def run(agent:MofaAgent):
    receive_data = agent.receive_parameters(['a_data','b_data','c_data'])
    # TODO: 在下面添加你的Agent代码,其中agent_inputs是你的Agent的需要输入的参数
    print("Received data:", receive_data)
    print("Sending data back:", receive_data)

    current_directory = os.getcwd()
    print("当前工作目录:", current_directory)
    #示例：调用一个函数处理接收到的数据
    model_name = 'gpt-3.5-turbo'  # 你可以根据需要更改模型名称
    prompt = f"Process the following data: {receive_data}"
    llm_response = choice_and_run_llm_model(model_name, prompt)
    print("LLM Response:", llm_response)

    agent_output_name = 'agent_result'
    agent.send_output(agent_output_name=agent_output_name,agent_result=receive_data)
def main():
    agent = MofaAgent(agent_name='you-agent-name')
    run(agent=agent)
if __name__ == "__main__":
    main()
