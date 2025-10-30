from mofa.agent_build.base.base_agent import MofaAgent, run_agent
import random

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    user_input = agent.receive_parameter('user_input')

    # Step 2: Implement the business logic
    result = str(random.randint(0, 10))

    # Step 3: Send output
    agent.send_output(agent_output_name='output', agent_result=result)

def main():
    agent = MofaAgent(agent_name='random-five-six')
    run(agent=agent)

if __name__ == "__main__":
    main()