import copy
import json
import os
from functools import wraps
import traceback
from os import mkdir
import pyarrow as pa
from attrs import define, field
from typing import Any, Dict, Union
import logging

from mcp.server import FastMCP

from mofa.utils.files.read import read_yaml
import yaml
from dora import Node
from dotenv import load_dotenv
from pathlib import Path
from mofa.utils.files.write import ensure_directory_exists
from logging.handlers import RotatingFileHandler

def create_agent_output(agent_name:str, agent_result:Union[str,dict,list], dataflow_status:bool=os.getenv(
                                                                                    "IS_DATAFLOW_END",                                                              True)):
    if isinstance(agent_result, dict) or isinstance(agent_result, list):
        agent_result = json.dumps(agent_result, ensure_ascii=False)
    return json.dumps({'step_name':agent_name, 'node_results':agent_result, 'dataflow_status':dataflow_status}, ensure_ascii=False)
def load_node_result(node_data):
    return json.loads(node_data).get('node_results')
@define
class MofaLogger:
    agent_name: str
    log_dir: str = field(default='logs')
    log_file: str = field(default='agent.log')
    max_log_size: int = field(default=10*1024*1024)  # 默认10MB
    backup_count: int = field(default=5)
    logger: logging.Logger = field(init=False, default=None)

    def __attrs_post_init__(self):
        """
        初始化 NodeLogger 实例。
        """
        self.logger = logging.getLogger(self.agent_name)
        self._setup_logger()

        if os.getenv('LOG_FILE', None) is not None:
            self.log_file = os.getenv('LOG_FILE')
            ensure_directory_exists(file_path=self.log_file)

    def _setup_logger(self):
        """
        设置日志记录器，包括日志目录的创建和日志处理器的配置。
        """
        # 创建日志目录（如果不存在）
        if not os.path.exists(self.log_dir):
            os.makedirs(self.log_dir, exist_ok=True)

        # 创建日志文件的完整路径
        log_path = os.path.join(self.log_dir, self.log_file)

        # 创建一个 RotatingFileHandler，用于日志轮转
        handler = RotatingFileHandler(
            log_path,
            maxBytes=self.max_log_size,
            backupCount=self.backup_count,
            encoding='utf-8'
        )
        handler.setLevel(logging.DEBUG)

        formatter = logging.Formatter('%(asctime)s - %(name)s - %(levelname)s - %(message)s')
        handler.setFormatter(formatter)

        # 将处理器添加到记录器
        self.logger.addHandler(handler)
        self.logger.setLevel(logging.DEBUG)

    def log(self,message:str, level:str='INFO' ):
        """
        记录日志消息。

        :param level: 日志级别，例如 'DEBUG', 'INFO', 'WARNING', 'ERROR', 'CRITICAL'
        :param message: 要记录的日志消息
        """
        log_method = getattr(self.logger, level.lower(), None)
        if log_method:
            log_method(message)
        else:
            self.logger.error(f"Invalid log level: {level}. Message: {message}")


@define
class MofaAgent:
    agent_name:str = field(default='mofa-agent')
    node:Any = field(default=None)
    agent_inputs:dict = field(factory=dict)
    event:Any = field(factory=dict)
    event_time_out:int = field(default=20)
    is_write_log:bool = field(default=False)
    log_file:str=field(default='agent.log')
    agent_log:MofaLogger = field(init=False)
    mcp :Any = field(default=None)
    def __attrs_post_init__(self):
        self.node = Node(self.agent_name)
        env_files = ['.env.secret', '.env']
        for env_file in env_files:
            if os.path.exists(env_file):
                load_dotenv(dotenv_path=env_file)
        log_params = ['IS_WRITE_LOG', 'WRITE_LOG']
        for log_status in log_params:
            if os.getenv(log_status, None) is not None:
                self.is_write_log = os.getenv(log_status)
        self.agent_log = MofaLogger(agent_name=self.agent_name, log_file=self.log_file)

    def __init_mcp(self):
        if os.getenv('MCP', None) is not None and self.mcp is None:
            self.mcp = FastMCP(self.agent_name)

    def register_mcp_tool(self, func):
        """动态注册 MCP 工具"""
        tool_name = func.__name__
        self.__init_mcp()
        decorated_func = self.mcp.tool()(func)
        # setattr(self, tool_name, decorated_func)
        print(f"工具 '{tool_name}' 注册成功。")

    def _parse_event_value(self, event):
        try:
            return load_node_result(event["value"][0].as_py())
        except Exception:
            return event["value"][0].as_py()

    def _receive_event_input(self, event, parameter_names:Union[str,dict]):
        if event["type"] == "INPUT":
            if isinstance(parameter_names,str):

                if event['id'] == parameter_names :
                    input_data = self._parse_event_value(event=event)
                    return input_data
            elif isinstance(parameter_names,dict):
                data = copy.deepcopy(parameter_names)
                if event['id'] in list(data.keys()) :
                    data[event['id']] = self._parse_event_value(event=event)
                    return data
                else:
                    return data
    def receive_parameter(self,parameter_name:str):
        for event in self.node:
            input_data = self._receive_event_input(event=event, parameter_names=parameter_name)
            if input_data is not None:
                self.event = event
                self.write_log(message=json.dumps(f"{self.agent_name}  receive  data : {input_data}  "))
                return input_data
            else:
                continue
            # self.node.next(self.event_time_out)

    def receive_parameters(self,parameter_names:list)->dict:
        parameter_data = {}
        if len(parameter_names) > 0:
            parameter_data = {key: None for key in parameter_names}
        for event in self.node:

            parameter_data = self._receive_event_input(event=event,parameter_names=parameter_data)
            self.event = event
            # self.node.next(self.event_time_out)
            is_parameter_data_status = all(value is not None for value in parameter_data.values())
            if is_parameter_data_status :
                break
        self.write_log(message=json.dumps(f"{self.agent_name}  receive parameters data : {parameter_data}  "))
        return parameter_data
            
    def send_output(self, agent_output_name: str, agent_result: Any, is_end_status=os.getenv('IS_DATAFLOW_END', True)):
        if is_end_status == 'true' or is_end_status == 'True':
            is_end_status = True
        self.node.send_output(
            agent_output_name,
            pa.array([create_agent_output(
                agent_name=agent_output_name,
                agent_result=agent_result,
                dataflow_status=is_end_status
            )]),
            self.event['metadata']
        )
        if agent_result == "None" or agent_result == " " or agent_result == "" or agent_result is None or agent_result == [] or agent_result == '[]':
            return
        self.write_log(message=json.dumps(f"{agent_output_name}  output data : {agent_result}  type : {type(agent_result)}" ))

    def write_log(self, message:str, level:str='INFO'):
        if self.is_write_log:
            if message == "None" or message == " " or message == "" or message is None or message == [] or message == '[]':
                return
            else:
                self.agent_log.log(message=message, level=level)
    def run_mcp(self,mcp_transport:str='sse'):
        if self.mcp is not None:
            print('mcp server 运行成功')
            self.mcp.run(transport=mcp_transport)


@define
class BaseMofaAgent:
    """
    A base class for MOFA agents, providing configuration management,
    resource initialization, and a context manager interface.
    """

    config_path: Union[str, None] = field(default=None)
    config: Dict[str, Any] = field(init=False,factory=dict)
    llm_client: Any = field(init=False, default=None)
    llm_config_path: Union[str, None] = field(default='.env.secret')
    
    def __attrs_post_init__(self) -> None:
        """
        Automatically called by attrs after the object is instantiated.
        Loads the configuration, initializes the client, 
        and sets up additional resources.
        """
        if self.config_path is None:
            self.config_path = os.path.join(os.path.abspath(os.path.dirname(__file__)), ) + '/configs/agent.yml'
        
    def init_llm_config(self,llm_config_path:str=None):
        if llm_config_path is None :
            llm_config_path = self.llm_config_path
        if Path(llm_config_path).exists():
            load_dotenv(llm_config_path)

    def load_config(self, config_path: str=None) -> Dict[str, Any]:
        """
        Load configuration from a YAML file.
        """
        if config_path is None:
            config_path = self.config_path
        return read_yaml(file_path = config_path)

    def send_output(self, node:Node,event,output_name: str, output_data: Any,output_step_name:str=None,is_end_dataflow:bool=True):
        if output_step_name is None:
            output_step_name = output_name
        node.send_output(output_name, pa.array([create_agent_output(agent_name=output_step_name, agent_result=output_data, dataflow_status=os.getenv('IS_DATAFLOW_END', is_end_dataflow))]), event['metadata'])
    
    def run(self,task:str=None,*args,**kwargs):
        pass
    
    def parse_agent_parameters(self,agent_input:Any):
        try:
            agent_input = load_node_result(agent_input)
        except:
            agent_input = agent_input
        return agent_input
    
    def create_llm_client(self,config:dict=None,*args,**kwargs):
        if config is None:
            config = self.config
        pass


def run_agent(func):
    @wraps(func)
    def wrapper(*args, **kwargs):
        while True:
            try:
                func(*args, **kwargs)
            except Exception as e:
                print(f"Error occurred: {e}")
                traceback.print_exc()
    return wrapper
