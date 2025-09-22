import importlib
import inspect
import os


def load_functions_from_directory(directory_path):
    functions_dict = {}

    for file in os.listdir(directory_path):
        if file.endswith('.py') and not file.startswith('__'):
            module_name = file[:-3]  # 移除.py后缀
            file_path = os.path.join(directory_path, file)

            spec = importlib.util.spec_from_file_location(module_name, file_path)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)

            functions_list = inspect.getmembers(module, inspect.isfunction)

            for func_name, func in functions_list:
                key = f"{module_name}_{func_name}"
                functions_dict[key] = func

    return functions_dict




def remove_duplicates_globally(input_dicts:list[dict]):
    result = []
    seen = set()

    for d in input_dicts:
        new_dict = {}
        for key, lst in d.items():
            unique_items = []
            for item in lst:
                if isinstance(item, list):
                    raise ValueError("Nested lists are not supported.")
                if item not in seen:
                    seen.add(item)
                    unique_items.append(item)

            new_dict[key] = unique_items

        result.append(new_dict)

    return result


