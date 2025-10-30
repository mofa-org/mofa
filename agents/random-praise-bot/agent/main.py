import random
from mofa.agent_build.base.base_agent import MofaAgent, run_agent

@run_agent
def run(agent: MofaAgent):
    # Step 1: Receive input parameter(s)
    agent.receive_parameter('user_input')

    # Step 2: Implement the business logic
    praises = [
        "You are absolutely amazing!",
        "Your brilliance knows no bounds!",
        "Incredible doesn't even begin to describe you!",
        "You radiate pure excellence!",
        "Your awesomeness is off the charts!",
        "You're a superstar in every way!",
        "Fantastic job, keep shining!",
        "You make the world a better place!",
        "Your greatness is truly inspiring!",
        "You're doing fantastic, keep it up!",
        "你太棒了！",
        "你真是才华横溢！",
        "不可思议已经无法形容你了！",
        "你散发着纯粹的卓越光芒！",
        "你的优秀爆表了！",
        "你全方位都是超级明星！",
        "干得好，继续闪耀吧！",
        "你让世界变得更美好！",
        "你的伟大真的令人鼓舞！",
        "你做得太棒了，继续保持！"
    ]
    result = random.choice(praises)

    # Step 3: Send output
    agent.send_output(agent_output_name='output', agent_result=result)

def main():
    agent = MofaAgent(agent_name='random-praise-bot')
    run(agent=agent)

if __name__ == "__main__":
    main()