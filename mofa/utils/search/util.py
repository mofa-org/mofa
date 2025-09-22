import json


def add_driver_cookies(driver, cookie_file_path:str):
    with open(cookie_file_path, "r") as f:
        cookies = json.load(f)
        print(cookies)
        for cookie in cookies:
            # Selenium 添加 Cookie 时不需要 'storeId', 'hostOnly', 'sameSite', 'session', 'path' 可以保留
            cookie_dict = {
                'domain': cookie.get('domain'),
                'name': cookie.get('name'),
                'value': cookie.get('value'),
                'path': cookie.get('path'),
                'secure': cookie.get('secure'),
                'httpOnly': cookie.get('httpOnly'),
            }

            # 处理 'expirationDate' 字段（如果存在）
            if 'expirationDate' in cookie:
                cookie_dict['expiry'] = int(cookie['expirationDate'])

            try:
                driver.add_cookie(cookie_dict)
                print(f"添加 Cookie: {cookie.get('name')}")
            except Exception as e:
                print(f"无法添加 Cookie: {cookie.get('name')}，原因: {e}")
    return driver