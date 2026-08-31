---
name: migrate
description: Create migration based on struct changes
---

1. Analyze the newly created or updated struct
2. Create migration files with command `sqlx migrate add -r <name>`
3. Discuss any decisions
4. Add the SQL migration code to the newly created files
