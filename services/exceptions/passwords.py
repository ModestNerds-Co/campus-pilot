#
#  passwords.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


class PasswordUsedException(Exception):
    def __init__(self, *args, **kwargs):
        self.args = args

    def __str__(self):
        return f"{self.args[0]}"
