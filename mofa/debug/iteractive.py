import click
import ast
from typing import List, Dict

def parse_value(value_str: str):
    """解析值为Python原生类型（支持字符串、列表、字典等）"""
    try:
        # 尝试解析为Python字面量（处理列表、字典等）
        return ast.literal_eval(value_str)
    except (SyntaxError, ValueError):
        # 解析失败则作为原始字符串返回
        return value_str

def collect_interactive_input() -> List[Dict]:
    """交互式收集测试用例，支持单项和列表形式的输入输出"""
    test_cases = []
    click.echo("===== 交互式测试用例输入 =====")
    click.echo("请按提示输入用例信息（格式与YAML保持一致）")
    click.echo("支持普通键值对（如 query:hello）和列表（如 parameter_names: [\"a\", \"b\", \"c\"]）\n")
    
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
        click.echo("\n请输入input参数（格式：key:value，例如：")
        click.echo("  普通值：query:hello")
        click.echo("  列表：parameter_names: [\"a\", \"b\", \"c\"]")
        input_str = click.prompt("input参数")
        input_dict = {}
        
        item = input_str.strip()
        if not item:
            continue
        if ":" not in item:
            raise click.BadParameter(f"输入格式错误：'{item}'，请使用 'key:value' 格式")
        key, value_str = item.split(":", 1)
        key = key.strip()
        value = parse_value(value_str.strip())  # 解析值为对应类型
        input_dict[key] = value
        case["input"] = input_dict
        
        # 3. 预期输出（expected_output）：同上支持多种类型
        click.echo("\n请输入预期输出（格式：key:value，例如：")
        click.echo("  普通值：hello_world_result:hello ")
        click.echo("  列表：receive_data: [\"a\", \"b\", \"c\"]")
        expected_output_str = click.prompt("expected_output参数")
        expected_output_dict = {}
        
        item = expected_output_str.strip()
        if not item:
            continue
        if ":" not in item:
            raise click.BadParameter(f"输入格式错误：'{item}'，请使用 'key:value' 格式")
        key, value_str = item.split(":", 1)
        key = key.strip()
        value = parse_value(value_str.strip())  # 解析值为对应类型
        expected_output_dict[key] = value
        case["expected_output"] = expected_output_dict
        
        # 添加到用例列表
        test_cases.append(case)
        case_index += 1
        
        # 询问是否继续添加
        if not click.confirm("\n是否继续添加下一个测试用例？", default=False):
            break
    
    click.echo(f"\n已收集 {len(test_cases)} 个测试用例，开始执行...")
    return test_cases
