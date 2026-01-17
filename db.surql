-- --- GUILD ---
-- record(user) devient record<user>
DEFINE FIELD owner ON TABLE guild TYPE record<user>; 

-- --- ROLE ---
DEFINE FIELD guild ON TABLE role TYPE record<guild>;

-- --- CHANNEL ---
DEFINE FIELD guild ON TABLE channel TYPE record<guild>;
-- option<string> était déjà bon, mais attention aux autres options

-- --- DM CHANNEL ---
DEFINE FIELD owner ON TABLE dm_channel TYPE record<user>;
DEFINE FIELD last_message_id ON TABLE dm_channel TYPE option<record<message>>;
DEFINE FIELD recipients.* ON TABLE dm_channel TYPE record<user>;

-- --- GUILD INVITE ---
DEFINE FIELD guild   ON TABLE guild_invite TYPE record<guild>;
DEFINE FIELD inviter ON TABLE guild_invite TYPE record<user>;

-- --- EMOJI ---
DEFINE FIELD owner ON TABLE emoji TYPE record<user>;
DEFINE FIELD guild ON TABLE emoji TYPE record<guild>;

-- --- MESSAGES ---
-- Attention ici pour les unions de types (channel OU dm_channel)
-- La syntaxe est record<table1 | table2>
DEFINE FIELD channel   ON TABLE message TYPE record<channel | dm_channel>;
DEFINE FIELD author    ON TABLE message TYPE record<user>;
DEFINE FIELD reply_to  ON TABLE message TYPE option<record<message>>;

-- --- MEMBER OF ---
DEFINE FIELD roles.* ON TABLE member_of TYPE record<role>;
