CREATE DATABASE IF NOT EXISTS acore_characters CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE DATABASE IF NOT EXISTS acore_world CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
GRANT ALL PRIVILEGES ON acore_auth.* TO 'acore'@'%';
GRANT ALL PRIVILEGES ON acore_characters.* TO 'acore'@'%';
GRANT ALL PRIVILEGES ON acore_world.* TO 'acore'@'%';
FLUSH PRIVILEGES;
