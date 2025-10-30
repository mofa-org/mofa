from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    feet = agent.receive_parameter('feet')

    # Step 2: Implement the business logic
    meters = float(feet) * 0.3048

    # Step 3: Send output
    agent.send_output(agent_output_name='meters', agent_result=meters)

def main():
    agent = MofaAgent(agent_name='feet-to-meters')
    run(agent=agent)

if __name__ == "__main__":
    main()