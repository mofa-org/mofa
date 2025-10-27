from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    task = agent.receive_parameter('task')
    # TODO: Add your agent code here
    # Process the task and generate result
    agent_result = task  # Replace with your logic

    agent.send_output(agent_output_name='agent_result', agent_result=agent_result)

def main():
    agent = MofaAgent(agent_name='{{cookiecutter.agent_name}}')
    run(agent=agent)

if __name__ == "__main__":
    main()
