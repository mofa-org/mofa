import click
import ast
from typing import List, Dict

# def parse_value(value_str: str):
#     """解析值为Python原生类型（支持字符串、列表、字典等）"""
#     try:
#         # 尝试解析为Python字面量（处理列表、字典等）
#         return ast.literal_eval(value_str)
#     except (SyntaxError, ValueError):
#         # 解析失败则作为原始字符串返回
#         return value_str

def collect_interactive_input(unit_test=True) -> List[Dict]:
    """support collect test cases interactively or user input mode."""
    if unit_test:
        test_cases = []
        click.echo("===== 交互式测试用例输入 =====")
        
        case_index = 1  # 用例序号，用于默认名称
        while True:
            case = {}
            
            # 1. 用例名称（默认自动生成，可自定义）
            default_name = f"test_case_{case_index}"
            case_name = click.prompt(
                f"请输入用例名称（默认: {default_name}）",
                type=str,
                default=default_name,
                show_default=False
            )
            case["name"] = case_name
            
            # 2. 输入参数（input）：支持key:value或key:[...]格式
            click.echo("\n请输入input参数")
            click.echo("  普通值如： hello")
            click.echo("  列表如： [\"a\", \"b\", \"c\"]")
            input_str = click.prompt("input参数")
            input_dict = {}
            
            item = input_str.strip()
            if not item:
                continue
            case["input"] = item
            
            # 3. 预期输出（expected_output）：同上支持多种类型
            click.echo("\n请输入预期输出")
            click.echo("  普通值如：hello ")
            click.echo("  列表如： [\"a\", \"b\", \"c\"]")
            expected_output_str = click.prompt("expected_output参数")
            
            item = expected_output_str.strip()
            if not item:
                continue
            case["expected_output"] = item
            
            test_cases.append(case)
            case_index += 1
            
            # Whether to continue adding more test cases
            if not click.confirm("\n是否继续添加下一个测试用例？", default=False):
                break
        
        click.echo(f"\n已收集 {len(test_cases)} 个测试用例，开始执行...")
        return test_cases
    else:
        # In non-unit-test mode, just collect a single input from user
        test_cases = []
        click.echo("===== 运行节点，输入参数 =====")
        input_str = click.prompt("请输入input参数")

        item = input_str.strip()
        if not item:
            raise click.BadParameter("输入不能为空")

        case = {
            "name": "user_input",
            "input": item
        }
        return [case]
