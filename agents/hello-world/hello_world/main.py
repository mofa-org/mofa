from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    user_query = agent.receive_parameter('query')
    # TODO
    print("Hello, World! Received query:", user_query)
    print("Hello, World! agent_result:", user_query)

    agent.send_output(agent_output_name='hello_world_result', agent_result=user_query)

def main():
    agent = MofaAgent(agent_name='hello-world')
    run(agent=agent)

if __name__ == "__main__":
    main()
