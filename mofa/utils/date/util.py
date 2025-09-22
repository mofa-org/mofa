from typing import Union

import pendulum
def now_time(format:Union[str,None]='YYYY-MM-DD HH:mm:ss'):
    """
    Get the current time and return it in the specified format.

    Parameters:
    format (Union[str, None]): Optional parameter specifying the format of the time. Default is 'YYYY-MM-DD HH:mm:ss'.

    Returns:
    str: The current time as a string in the specified format.
    """
    return pendulum.now().format(format)


