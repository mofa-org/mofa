from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    dog_name = agent.receive_parameter('dog_name')

    # Step 2: Implement the business logic
    dog_char_art = f"""
 / \__
(    @\__ 
 /         O
/   (_____/
/_____/ U
    """
    dog_char_art_with_name = f"{dog_name}\n{dog_char_art}"

    # Step 3: Send output
    agent.send_output(agent_output_name='dog_char_art', agent_result=dog_char_art_with_name)

def main():
    agent = MofaAgent(agent_name='puppy-char-drawer')
    run(agent=agent)

if __name__ == "__main__":
    main()