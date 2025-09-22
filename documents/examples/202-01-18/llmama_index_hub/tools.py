import openai
import os
from dotenv import load_dotenv
from llama_index.agent.openai import OpenAIAgent
from llama_index.tools.arxiv.base import ArxivToolSpec
from llama_index.tools.duckduckgo import DuckDuckGoSearchToolSpec
import importlib.util
from typing import List, Dict, Optional
from llama_index.core.tools.tool_spec.base import BaseToolSpec


class MofaExampleLlamaHubTool(BaseToolSpec):
    """MofaExampleLlamaHubTool tool spec."""

    spec_functions = ["calculate",]

    def calculate(self, num_one: int,num_two:int) -> int:
        """
        Calculates a value based on two input integers.

        The function performs the following operations:
        1. Multiplies the first integer (num_one) by the second integer (num_two).
        2. Adds the first integer (num_one) to the result of the multiplication.
        3. Computes the sum of the two integers (num_two + num_one).
        4. Performs integer division of the sum by the second integer (num_two).
        5. Subtracts the result of the division from the earlier sum.

        The final result is returned as an integer.

        Parameters:
        num_one (int): The first input integer.
        num_two (int): The second input integer.

        Returns:
        int: The calculated result.
        """
        return  num_one * num_two + num_one - (num_two + num_one) // num_two

tools = MofaExampleLlamaHubTool()
tools.get_metadata_from_fn_name("calculate")
# 加载.env文件
# load_dotenv('.env')

# 从环境变量中获取API key
api_key = ''
if api_key:
    openai.api_key = api_key
else:
    raise ValueError("未找到OPENAI_API_KEY环境变量，请检查.env文件配置")

openai.api_key = api_key
duckduckgo_tool  = DuckDuckGoSearchToolSpec()

arxiv_tool = ArxivToolSpec()

# agent = OpenAIAgent.from_tools(
#     arxiv_tool.to_tool_list(),
#     verbose=True,
# )
agent = OpenAIAgent.from_tools(
    MofaExampleLlamaHubTool().to_tool_list(),
    verbose=True,
)
agent_result = agent.chat("我有两个数字，分别是10和15，计算它们的结果")
print(agent_result)

