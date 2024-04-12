#
#  generate_random_text.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.


import random
import string


def generate_random_text(n: int = 35) -> str:
    characters = string.ascii_letters + string.digits
    random_text = "".join(random.choice(characters) for _ in range(n))
    return random_text
