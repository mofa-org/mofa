from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    expression = agent.receive_parameter('expression')

    # Step 2: Implement the business logic
    if not expression or not expression.strip():
        result = 0
    else:
        try:
            result = eval(expression, {"__builtins__": {}}, {})
        except Exception:
            result = 0

    # Step 3: Send output
    agent.send_output(agent_output_name='result', agent_result=result)

def main():
    agent = MofaAgent(agent_name='expr-calculator')
    run(agent=agent)

if __name__ == "__main__":
    main()