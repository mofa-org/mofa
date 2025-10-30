from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    n_input = agent.receive_parameter('n')

    # Type conversion and validation
    try:
        n = int(n_input)
        if n < 0:
            agent.send_output(agent_output_name='sequence',
                            agent_result={'error': 'Please enter a non-negative number'})
            return
    except (ValueError, TypeError):
        agent.send_output(agent_output_name='sequence',
                        agent_result={'error': f'Invalid input "{n_input}". Please enter a valid number.'})
        return

    # Step 2: Implement the business logic
    if n == 0:
        sequence = []
    else:
        sequence = [0]
        a, b = 0, 1
        for _ in range(1, n):
            sequence.append(b)
            a, b = b, a + b

    # Step 3: Send output
    agent.send_output(agent_output_name='sequence', agent_result=sequence)

def main():
    agent = MofaAgent(agent_name='fibonacci-generator')
    run(agent=agent)

if __name__ == "__main__":
    main()