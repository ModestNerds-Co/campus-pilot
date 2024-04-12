#
#  __init__.py
#  campus-pilot
#
#  Created by Ngonidzashe Mangudya on 12/04/2024.
#  Copyright (c) 2024 Codecraft Solutions. All rights reserved.

import os
from campuspilot.settings.base import *

PRODUCTION = os.path.exists("production.txt")
STAGING = os.path.exists("staging.txt")

if PRODUCTION:
    if STAGING:
        from campuspilot.settings.staging import *
    else:
        from campuspilot.settings.production import *

else:
    from campuspilot.settings.development import *
