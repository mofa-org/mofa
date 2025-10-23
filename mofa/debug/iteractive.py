import click
from typing import List, Dict

def collect_interactive_input() -> List[Dict]:
    """交互式收集测试用例，格式与YAML保持一致（包含name、input、expected_output）"""
    test_cases = []
    click.echo("===== 交互式测试用例输入 =====")
    click.echo("请按提示输入用例信息（格式与YAML保持一致）\n")
    
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
        
        # 2. 输入参数（input）：解析为 {"query": ...} 格式
        click.echo("\n请输入input参数（格式：key:value，键值对形式，例如 query:hello ）")
        input_str = click.prompt("input参数")
        input_dict = {}
        for item in input_str.split(","):
            item = item.strip()
            if not item:
                continue
            # 分割为key和value（只按第一个":"分割）
            if ":" not in item:
                raise click.BadParameter(f"输入格式错误：'{item}'，请使用 'key:value' 格式")
            key, value = item.split(":", 1)
            input_dict[key.strip()] = value.strip()
        case["input"] = input_dict  # 与YAML的input结构一致
        
        # 3. 预期输出（expected_output）：同样解析为字典格式
        click.echo("\n请输入预期输出（格式：key:value， 例如 hello_world_result:hello ）")
        expected_output_str = click.prompt("expected_output参数")
        expected_output_dict = {}
        for item in expected_output_str.split(","):
            item = item.strip()
            if not item:
                continue
            if ":" not in item:
                raise click.BadParameter(f"输入格式错误：'{item}'，请使用 'key:value' 格式")
            key, value = item.split(":", 1)
            expected_output_dict[key.strip()] = value.strip()
        case["expected_output"] = expected_output_dict  # 与YAML的expected_output结构一致
        
        # 添加到用例列表
        test_cases.append(case)
        case_index += 1
        
        # 询问是否继续添加
        if not click.confirm("\n是否继续添加下一个测试用例？", default=False):
            break
    
    click.echo(f"\n已收集 {len(test_cases)} 个测试用例，开始执行...")
    return test_cases
