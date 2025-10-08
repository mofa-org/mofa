import ast
import os
import sys
import uuid
import importlib.util
from typing import Dict, List, Tuple, Optional

def extract_agent_info(code: str) -> Dict:
    """
    parse the provided code to extract:
    1. Parameters passed to agent.receive_parameter
    2. Parameters passed to agent.send_output
    3. Code between these two calls
    """
    tree = ast.parse(code)
    
    receive_nodes = []
    send_nodes = []
    
    class AgentCallVisitor(ast.NodeVisitor):
        def visit_Assign(self, node):
            #（ a = agent.receive_parameter(...)）
            if isinstance(node.value, ast.Call):
                self._check_call_node(node.value, node)
            self.generic_visit(node)
            
        def visit_Expr(self, node):
            #（ agent.send_output(...)）
            if isinstance(node.value, ast.Call):
                self._check_call_node(node.value, node)
            self.generic_visit(node)
            
        def _check_call_node(self, call_node: ast.Call, parent_node):
            # Check if the call is to agent.receive_parameter or agent.send_output
            if (isinstance(call_node.func, ast.Attribute) and
                isinstance(call_node.func.value, ast.Name) and
                call_node.func.value.id == 'agent'):
                
                func_name = call_node.func.attr
                if func_name == 'receive_parameter':
                    receive_nodes.append((parent_node, call_node))
                elif func_name == 'send_output':
                    send_nodes.append((parent_node, call_node))
    
    # extract calls
    visitor = AgentCallVisitor()
    visitor.visit(tree)
    
    # Extract arguments from a call node
    def get_call_args(call_node: ast.Call) -> List[str]:
        args = []
        # dealing with positional arguments
        for arg in call_node.args:
            if isinstance(arg, ast.Str):
                # add quotes for string literals
                args.append(f"'{arg.s}'")
            else:
                args.append(ast.unparse(arg))
        # dealing with keyword arguments
        for kw in call_node.keywords:
            value_str = ast.unparse(kw.value)
            # add quotes for string literals
            if isinstance(kw.value, ast.Str):
                value_str = f"'{kw.value.s}'"
            args.append(f"{kw.arg}={value_str}")
        return args
    
    receive_params = []
    if receive_nodes:
        receive_params = get_call_args(receive_nodes[0][1])
    
    send_params = []
    if send_nodes:
        send_params = get_call_args(send_nodes[0][1])
    
    # Extract code between the two calls
    between_code = ""
    if receive_nodes and send_nodes:
        receive_parent, _ = receive_nodes[0]
        send_parent, _ = send_nodes[0]
        
        # Ensure the receive call comes before the send call
        if receive_parent.lineno < send_parent.lineno:
            # Calculate the line range between the two calls
            start_line = receive_parent.end_lineno + 1
            end_line = send_parent.lineno - 1
            
            if start_line <= end_line:
                # Split the code into lines and extract the middle part
                code_lines = code.split('\n')
                # Handle potential empty lines
                between_lines = []
                for line_num in range(start_line-1, end_line):
                    if line_num < len(code_lines):
                        between_lines.append(code_lines[line_num].rstrip('\r'))
                
                between_code = '\n'.join(between_lines).strip()
    
    return {
        "receive_params": receive_params,
        "send_params": send_params,
        "between_code": between_code
    }

if __name__ == "__main__":
    # 示例代码（使用用户提供的原始代码）
    sample_code = """
@run_agent
def run(agent: MofaAgent):
    user_query = agent.receive_parameter('query')
    # TODO
    agent.send_output(agent_output_name='hello_world_result', agent_result=user_query)
    """
    
    result = extract_agent_info(sample_code)
    
    print("receive_parameter参数:", result["receive_params"])
    print("send_output参数:", result["send_params"])
    print("中间代码内容:\n", result["between_code"])
    


def load_node_module(node_folder_path: str, execute: bool = False):
    """
    Dynamically load a Python module for a node/agent located in a folder.

    Strategy:
    - Walk the folder and prefer a file named `main.py` if present.
    - Otherwise pick the first .py file at the top level.
    - Import it as a unique module name and return the module object.

    Raises ImportError on failure.
    """
    node_folder_path = os.path.abspath(node_folder_path)

    if not os.path.exists(node_folder_path):
        raise ImportError(f"Node folder not found: {node_folder_path}")

    candidate = None
    for root, dirs, files in os.walk(node_folder_path):
        # prefer main.py
        if 'main.py' in files:
            candidate = os.path.join(root, 'main.py')
            break
        # otherwise, continue walking but prefer shallow files
        for f in files:
            if f.endswith('.py'):
                candidate = os.path.join(root, f)
                # don't break here: prefer main.py if found in later iteration

    if candidate is None:
        # fallback: check top-level .py files
        try:
            for f in os.listdir(node_folder_path):
                if f.endswith('.py'):
                    candidate = os.path.join(node_folder_path, f)
                    break
        except Exception:
            pass

    if candidate is None:
        raise ImportError(f"No python module found in {node_folder_path}")

    # Read source to allow analysis (check for main and extract agent info)
    try:
        with open(candidate, 'r', encoding='utf-8') as _f:
            source_code = _f.read()
    except Exception as e:
        raise ImportError(f"Failed to read source file {candidate}: {e}") from e

    # If the source defines a function named `main`, call extract_agent_info on the source
    agent_info = None
    try:
        parsed = ast.parse(source_code)
        has_main = any(isinstance(n, ast.FunctionDef) and n.name == 'main' for n in parsed.body)
        if has_main:
            try:
                agent_info = extract_agent_info(source_code)
            except Exception:
                # don't block module import if extraction fails; keep agent_info as None
                agent_info = None
    except Exception:
        # If parsing fails, continue to attempt importing the module
        agent_info = None
    # If execute is False, do not import/execute the module — return a descriptor for later testing.
    descriptor = {
        'path': candidate,
        'source': source_code,
        'agent_info': agent_info,
    }

    if not execute:
        print(f"Loaded node module descriptor ：{descriptor} \n \n \n")
        return descriptor

    # If execution is requested, perform dynamic import as before and attach agent_info
    module_name = f"mofa_debug_loaded_{uuid.uuid4().hex}"
    spec = importlib.util.spec_from_file_location(module_name, candidate)
    if spec is None or spec.loader is None:
        raise ImportError(f"Unable to create import spec for {candidate}")

    module = importlib.util.module_from_spec(spec)
    try:
        # register and execute module
        sys.modules[module_name] = module
        spec.loader.exec_module(module)
    except Exception as e:
        # ensure we don't leave a broken module in sys.modules
        if module_name in sys.modules:
            del sys.modules[module_name]
        raise ImportError(f"Failed to import node module from {candidate}: {e}") from e

    # attach agent_info if extraction was successful
    if agent_info is not None:
        try:
            setattr(module, 'agent_info', agent_info)
        except Exception:
            # ignore failures to attach
            pass

    return module


__all__ = ["extract_agent_info", "load_node_module"]
