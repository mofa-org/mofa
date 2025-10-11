from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent:MofaAgent):
    receive_data = agent.receive_parameters(['a_data','b_data','c_data'])
    # TODO: 在下面添加你的Agent代码,其中agent_inputs是你的Agent的需要输入的参数
    print("Received data:", receive_data)
    print("Sending data back:", receive_data)

    agent_output_name = 'agent_result'
    agent.send_output(agent_output_name=agent_output_name,agent_result=receive_data)
def main():
    agent = MofaAgent(agent_name='you-agent-name')
    run(agent=agent)
if __name__ == "__main__":
    main()
