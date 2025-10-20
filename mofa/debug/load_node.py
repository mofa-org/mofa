import ast
import os
import sys
import importlib
from typing import Dict, List, Set, Any


def remove_main_guard(raw_code):
    """Fixed code by removing the if __name__ == "__main__" guard and its main() call."""
    lines = raw_code.split('\n')
    new_lines = []
    skip = False
    # Support both spaced and unspaced versions
    target_patterns = [
        'if __name__=="__main__":',
        'if __name__ == "__main__":'
    ]
    
    for line in lines:
        stripped_line = line.strip()
        # Check for the main guard line
        if stripped_line in target_patterns:
            skip = True
            continue
        # If we are in a skip state, check for the main() call
        if skip:
            # Skip the main() call line and end the skip state afterwards
            if stripped_line == 'main()':
                skip = False
                continue
            # Skip empty lines or other indented lines (adjust based on actual code formatting)
            if not stripped_line:
                continue
        # If not skipping, keep the line
        new_lines.append(line)
    
    return '\n'.join(new_lines)

class DependencyAnalyzer(ast.NodeVisitor):
    def __init__(self):
        self.imported_modules = set()
        self.imported_objects = dict()
        self.global_functions = set()
        self.global_variables = set()
        self.relative_imports = set()

    def visit_Import(self, node):
        for alias in node.names:
            self.imported_modules.add(alias.name)
        self.generic_visit(node)

    def visit_ImportFrom(self, node):
        module = node.module or ''
        for alias in node.names:
            self.imported_objects[alias.asname or alias.name] = (module, alias.name)
        if module:
            self.imported_modules.add(module)
        # Track relative imports
        if node.level > 0:
            self.relative_imports.add((node.level, module))
        self.generic_visit(node)

    def visit_FunctionDef(self, node):
        self.global_functions.add(node.name)
        self.generic_visit(node)

    def visit_Assign(self, node):
        if isinstance(getattr(node, 'parent', None), ast.Module):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    self.global_variables.add(target.id)
        self.generic_visit(node)

class ParentSetter(ast.NodeVisitor):
    def visit(self, node):
        for child in ast.iter_child_nodes(node):
            child.parent = node
            self.visit(child)

def analyze_dependencies(code: str) -> Dict:
    tree = ast.parse(code)
    ParentSetter().visit(tree)
    analyzer = DependencyAnalyzer()
    analyzer.visit(tree)
    return {
        "imported_modules": analyzer.imported_modules,
        "imported_objects": analyzer.imported_objects,
        "global_functions": analyzer.global_functions,
        "global_variables": analyzer.global_variables,
        "relative_imports": analyzer.relative_imports
    }

def normalize_relative_imports(source_code: str) -> str:
    """
    Normalize relative imports (from .xxx / from ..xxx) by setting level to 0
    while keeping module name unchanged. This makes code exec-able when there is
    no package context (module_path is None). Falls back to original source on
    any failure.
    """
    try:
        tree = ast.parse(source_code)
        class RelImportRewriter(ast.NodeTransformer):
            def visit_ImportFrom(self, node):
                if getattr(node, 'level', 0) and node.module is not None:
                    # Drop relative level but keep module name
                    return ast.copy_location(
                        ast.ImportFrom(module=node.module, names=node.names, level=0),
                        node
                    )
                return node
        new_tree = RelImportRewriter().visit(tree)
        ast.fix_missing_locations(new_tree)
        return ast.unparse(new_tree)
    except Exception:
        return source_code

def build_execution_globals(source_code: str, module_path: str = None) -> Dict[str, Any]:
    execution_globals = {
        "__builtins__": __builtins__,
        "__name__": "__main__",
        "__file__": module_path or "",
        "__package__": os.path.basename(os.path.dirname(module_path)) if module_path else None  # set package name if path provided
    }
    deps = analyze_dependencies(source_code)
    
    # Handle relative imports when module_path is known
    if deps["relative_imports"] and module_path:
        module_dir = os.path.dirname(os.path.abspath(module_path))
        parent_dir = os.path.dirname(module_dir)
        if parent_dir not in sys.path:
            sys.path.insert(0, parent_dir)

    # Import modules
    for module_name in deps["imported_modules"]:
        module = None
        # 1) Try absolute import first
        try:
            module = importlib.import_module(module_name)
        except ImportError:
            pass

        # 2) If failed and package context exists, try package-relative import
        if module is None and module_path and execution_globals.get("__package__"):
            try:
                module = importlib.import_module(module_name, package=execution_globals["__package__"])
            except ImportError:
                pass

        # 3) Finally try explicit dotted-relative form (from .xxx)
        if module is None and module_path and execution_globals.get("__package__"):
            try:
                module = importlib.import_module(f".{module_name}", package=execution_globals["__package__"])
            except ImportError:
                pass

        if module is not None:
            execution_globals[module_name] = module
        else:
            print(f"警告：模块 {module_name} 导入失败，可能导致执行错误")

    for alias, (module_name, obj_name) in deps["imported_objects"].items():
        module = None
        # If module was already loaded and contains the object, use it
        if module_name in execution_globals and hasattr(execution_globals[module_name], obj_name):
            execution_globals[alias] = getattr(execution_globals[module_name], obj_name)
            continue

        # 1) Absolute import
        try:
            module = importlib.import_module(module_name)
        except ImportError:
            module = None

        # 2) Package-relative import
        if module is None and module_path and execution_globals.get("__package__"):
            try:
                module = importlib.import_module(module_name, package=execution_globals["__package__"])
            except ImportError:
                module = None

        # 3) Dotted-relative import
        if module is None and module_path and execution_globals.get("__package__"):
            try:
                module = importlib.import_module(f".{module_name}", package=execution_globals["__package__"])
            except ImportError:
                module = None

        if module is not None:
            try:
                execution_globals[alias] = getattr(module, obj_name)
            except AttributeError:
                print(f"警告：对象 {obj_name} 从模块 {module_name} 导入失败")
        else:
            print(f"警告：对象 {obj_name} 从模块 {module_name} 导入失败")

    # Load global functions and variables
    temp_globals = execution_globals.copy()
    try:
        code_to_exec = source_code
        # If module_path is not provided, normalize relative imports to avoid
        # "no known parent package"
        if not module_path and deps["relative_imports"]:
            code_to_exec = normalize_relative_imports(source_code)
        exec(code_to_exec, temp_globals)
        for func_name in deps["global_functions"]:
            if func_name in temp_globals:
                execution_globals[func_name] = temp_globals[func_name]
        for var_name in deps["global_variables"]:
            if var_name in temp_globals:
                execution_globals[var_name] = temp_globals[var_name]
    except Exception as e:
        print(f"Error while executing source code to load globals: {e}")

    return execution_globals

def extract_agent_info(code: str) -> Dict:
    tree = ast.parse(code)
    
    receive_nodes = []
    send_nodes = []
    
    class AgentCallVisitor(ast.NodeVisitor):
        def visit_Assign(self, node):
            if isinstance(node.value, ast.Call):
                self._check_call_node(node.value, node)
            self.generic_visit(node)
            
        def visit_Expr(self, node):
            if isinstance(node.value, ast.Call):
                self._check_call_node(node.value, node)
            self.generic_visit(node)
            
        def _check_call_node(self, call_node: ast.Call, parent_node):
            if (isinstance(call_node.func, ast.Attribute) and
                isinstance(call_node.func.value, ast.Name) and
                call_node.func.value.id == 'agent'):
                
                func_name = call_node.func.attr
                # Accept common variants: singular, plural and short names
                if func_name in ('receive_parameter', 'receive_parameters', 'receive_params', 'receive'):
                    receive_nodes.append((parent_node, call_node))
                elif func_name == 'send_output':
                    send_nodes.append((parent_node, call_node))
    
    visitor = AgentCallVisitor()
    visitor.visit(tree)
    
    def get_call_args(call_node: ast.Call) -> List[str]:
        args = []
        for arg in call_node.args:
            # If the argument is a list/tuple literal of string constants, expand it
            if isinstance(arg, (ast.List, ast.Tuple)):
                all_consts = True
                for elt in arg.elts:
                    if not isinstance(elt, ast.Constant) or not isinstance(elt.value, str):
                        all_consts = False
                        break
                if all_consts:
                    for elt in arg.elts:
                        args.append(f"'{elt.value}'")
                    continue

            if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
                args.append(f"'{arg.value}'")
            else:
                args.append(ast.unparse(arg))
        for kw in call_node.keywords:
            value_str = ast.unparse(kw.value)
            if isinstance(kw.value, ast.Constant) and isinstance(kw.value.value, str):
                value_str = f"'{kw.value.value}'"
            args.append(f"{kw.arg}={value_str}")
        return args
    
    receive_params = []
    if receive_nodes:
        receive_params = get_call_args(receive_nodes[0][1])
    
    receive_target = None
    if receive_nodes:
        parent = receive_nodes[0][0]
        try:
            if isinstance(parent, ast.Assign):
                targets = parent.targets
                if targets:
                    t = targets[0]
                    if isinstance(t, ast.Name):
                        receive_target = t.id
                    elif isinstance(t, ast.Tuple):
                        for elt in t.elts:
                            if isinstance(elt, ast.Name):
                                receive_target = elt.id
                                break
            elif isinstance(parent, ast.AnnAssign):
                t = parent.target
                if isinstance(t, ast.Name):
                    receive_target = t.id
        except Exception:
            receive_target = None
    
    send_params = []
    if send_nodes:
        send_params = get_call_args(send_nodes[0][1])
    
    between_code = ""
    if receive_nodes and send_nodes:
        receive_parent, _ = receive_nodes[0]
        send_parent, _ = send_nodes[0]
        
        if receive_parent.lineno < send_parent.lineno:
            start_line = receive_parent.end_lineno + 1
            end_line = send_parent.lineno - 1
            
            if start_line <= end_line:
                code_lines = code.split('\n')
                between_lines = []
                for line_num in range(start_line-1, end_line):
                    if line_num < len(code_lines):
                        line = code_lines[line_num].rstrip('\r')
                        if line.strip():
                            between_lines.append(line)
                
                if between_lines:
                    min_indent = min(len(line) - len(line.lstrip()) for line in between_lines)
                    between_lines = [line[min_indent:] for line in between_lines]
                    between_code = '\n'.join(between_lines).strip()
    
    return {
        "receive_params": receive_params,
        "receive_target": receive_target,
        "send_params": send_params,
        "between_code": between_code
    }

def load_node_module(node_folder_path: str, execute: bool = False):
    node_folder_path = os.path.abspath(node_folder_path)

    if not os.path.exists(node_folder_path):
        raise ImportError(f"Node folder not found: {node_folder_path}")

    candidate = None
    for root, dirs, files in os.walk(node_folder_path):
        if 'main.py' in files:
            candidate = os.path.join(root, 'main.py')
            break
        for f in files:
            if f.endswith('.py'):
                candidate = os.path.join(root, f)

    if candidate is None:
        try:
            for f in os.listdir(node_folder_path):
                if f.endswith('.py'):
                    candidate = os.path.join(node_folder_path, f)
                    break
        except Exception:
            pass

    if candidate is None:
        raise ImportError(f"No python module found in {node_folder_path}")

    try:
        with open(candidate, 'r', encoding='utf-8') as _f:
            source_code = _f.read()
    except Exception as e:
        raise ImportError(f"Failed to read source file {candidate}: {e}") from e

    agent_info = None
    try:
        parsed = ast.parse(source_code)
        has_main = any(isinstance(n, ast.FunctionDef) and n.name == 'main' for n in parsed.body)
        if has_main:
            try:
                agent_info = extract_agent_info(source_code)
            except Exception:
                agent_info = None
    except Exception:
        agent_info = None

    execution_globals = None
    if execute:
        execution_globals = build_execution_globals(source_code, candidate)

    descriptor = {
        'path': candidate,
        'source': source_code,
        'agent_info': agent_info,
        'execution_globals': execution_globals
    }

    if not execute:
        return descriptor
    
    try:
        exec(source_code, execution_globals)
    except Exception as e:
        print(f"Error executing module: {e}")
    return descriptor

if __name__ == "__main__":
    sample_code = """
from mofa.agent_build.base.base_agent import MofaAgent, run_agent
from openai import OpenAI
import os

from .multi_llm import choice_and_run_llm_model  # 相对导入

def identify_info_types(user_input):
    info_types = []
    medicine_keywords = ["药", "吃药"]
    if any(keyword in user_input for keyword in medicine_keywords):
        info_types.append("用药")
    return info_types if info_types else ["其他"]

@run_agent
def run(agent: MofaAgent):
    user_query = agent.receive_parameter('query')
    types = identify_info_types(user_query)
    result = f"识别到信息类型: {types}"
    agent.send_output(agent_output_name='result', agent_result=result)
    """
    
    deps = analyze_dependencies(sample_code)
    print("Imported modules:", deps["imported_modules"])
    print("Imported objects:", deps["imported_objects"])
    print("Global functions:", deps["global_functions"])
    
    exec_globals = build_execution_globals(sample_code)
    print("\nExecution globals contain:", list(exec_globals.keys()))
    
    agent_info = extract_agent_info(sample_code)
    print("\nBetween code:\n", agent_info["between_code"])
    
    if agent_info["between_code"]:
        try:
            exec_globals["agent"] = None
            exec_globals["user_query"] = "我需要吃什么药？"
            
            # Fix typo: use exec_globals
            if "identify_info_types" not in exec_globals:
                print("Warning: identify_info_types not loaded, trying reload...")
                temp_globals = {**exec_globals}
                # Normalize relative imports for demo code
                demo_code = normalize_relative_imports(sample_code)
                exec(demo_code, temp_globals)
                exec_globals["identify_info_types"] = temp_globals.get("identify_info_types")
            
            exec(agent_info["between_code"], exec_globals)
            print("Execute success, result:", exec_globals.get("result"))
        except Exception as e:
            print("Error executing between code:", e)

__all__ = ["extract_agent_info", "load_node_module", "analyze_dependencies", "build_execution_globals"]
