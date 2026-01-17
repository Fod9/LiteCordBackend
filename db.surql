DEFINE TABLE user SCHEMAFULL;
    DEFINE FIELD name            ON TABLE user TYPE string;
    DEFINE FIELD display_name    ON TABLE user TYPE string;
    DEFINE FIELD profile_picture ON TABLE user TYPE string;
    DEFINE FIELD email           ON TABLE user TYPE string ASSERT string::is::email($value);
    DEFINE FIELD password        ON TABLE user TYPE string;
    DEFINE FIELD status          ON TABLE user TYPE string ASSERT $value INSIDE ['Online', 'Offline', 'Invisible', 'DoNotDisturb'];
    DEFINE FIELD created_at      ON TABLE user TYPE datetime DEFAULT time::now();

    DEFINE INDEX user_email      ON TABLE user COLUMNS email UNIQUE;
    DEFINE INDEX user_name       ON TABLE user COLUMNS name UNIQUE;

DEFINE TABLE guild SCHEMAFULL;
    DEFINE FIELD name            ON TABLE guild TYPE string;
    DEFINE FIELD icon            ON TABLE guild TYPE string;
    DEFINE FIELD owner           ON TABLE guild TYPE record<user>;
    DEFINE FIELD created_at      ON TABLE guild TYPE datetime DEFAULT time::now();

DEFINE TABLE role SCHEMAFULL;
    DEFINE FIELD guild           ON TABLE role TYPE record<guild>;
    DEFINE FIELD name            ON TABLE role TYPE string;
    DEFINE FIELD color           ON TABLE role TYPE string;
    DEFINE FIELD position        ON TABLE role TYPE int;
    DEFINE FIELD permissions     ON TABLE role TYPE array;
    DEFINE FIELD permissions.* ON TABLE role TYPE string;

    DEFINE INDEX role_guild      ON TABLE role COLUMNS guild;

DEFINE TABLE channel SCHEMAFULL;
    DEFINE FIELD guild           ON TABLE channel TYPE record<guild>;
    DEFINE FIELD name            ON TABLE channel TYPE string;
    DEFINE FIELD channel_type    ON TABLE channel TYPE string ASSERT $value INSIDE ['Text', 'Voice'];
    DEFINE FIELD category        ON TABLE channel TYPE option<string>;
    DEFINE FIELD created_at      ON TABLE channel TYPE datetime DEFAULT time::now();

    DEFINE INDEX channel_guild   ON TABLE channel COLUMNS guild;

DEFINE TABLE dm_channel SCHEMAFULL;
    DEFINE FIELD name            ON TABLE dm_channel TYPE option<string>;
    DEFINE FIELD owner           ON TABLE dm_channel TYPE record<user>;
    DEFINE FIELD last_message_id ON TABLE dm_channel TYPE option<record<message>>;
    DEFINE FIELD created_at      ON TABLE dm_channel TYPE datetime DEFAULT time::now();
    DEFINE FIELD recipients      ON TABLE dm_channel TYPE array;
    DEFINE FIELD recipients.* ON TABLE dm_channel TYPE record<user>;

DEFINE TABLE guild_invite SCHEMAFULL;
    DEFINE FIELD guild           ON TABLE guild_invite TYPE record<guild>;
    DEFINE FIELD inviter         ON TABLE guild_invite TYPE record<user>;
    DEFINE FIELD code            ON TABLE guild_invite TYPE string;
    DEFINE FIELD expires_at      ON TABLE guild_invite TYPE option<datetime>;
    DEFINE FIELD created_at      ON TABLE guild_invite TYPE datetime DEFAULT time::now();

    DEFINE INDEX invite_code     ON TABLE guild_invite COLUMNS code UNIQUE;

DEFINE TABLE emoji SCHEMAFULL;
    DEFINE FIELD owner           ON TABLE emoji TYPE record<user>;
    DEFINE FIELD guild           ON TABLE emoji TYPE record<guild>;
    DEFINE FIELD name            ON TABLE emoji TYPE string;
    DEFINE FIELD image           ON TABLE emoji TYPE string;
    DEFINE FIELD created_at      ON TABLE emoji TYPE datetime DEFAULT time::now();

    DEFINE INDEX emoji_guild     ON TABLE emoji COLUMNS guild;

DEFINE TABLE message SCHEMAFULL;
    DEFINE FIELD channel         ON TABLE message TYPE record<channel | dm_channel>;
    DEFINE FIELD author          ON TABLE message TYPE record<user>;
    DEFINE FIELD content         ON TABLE message TYPE string;
    DEFINE FIELD reply_to        ON TABLE message TYPE option<record<message>>;
    DEFINE FIELD edited_at       ON TABLE message TYPE option<datetime>;
    DEFINE FIELD created_at      ON TABLE message TYPE datetime DEFAULT time::now();
    DEFINE FIELD attachments     ON TABLE message TYPE array;
    DEFINE FIELD attachments.* ON TABLE message TYPE object;
    DEFINE FIELD attachments.*.url      ON TABLE message TYPE string;
    DEFINE FIELD attachments.*.filename ON TABLE message TYPE string;
    DEFINE FIELD attachments.*.size     ON TABLE message TYPE int;

    DEFINE INDEX message_channel ON TABLE message COLUMNS channel;

DEFINE TABLE member_of SCHEMAFULL TYPE RELATION IN user OUT guild;
    DEFINE FIELD nickname        ON TABLE member_of TYPE option<string>;
    DEFINE FIELD joined_at       ON TABLE member_of TYPE datetime DEFAULT time::now();
    DEFINE FIELD roles           ON TABLE member_of TYPE array;
    DEFINE FIELD roles.* ON TABLE member_of TYPE record<role>;

    DEFINE INDEX unique_membership ON TABLE member_of COLUMNS in, out UNIQUE;

DEFINE TABLE friendship SCHEMAFULL TYPE RELATION IN user OUT user;
    DEFINE FIELD created_at      ON TABLE friendship TYPE datetime DEFAULT time::now();

    DEFINE INDEX unique_friendship ON TABLE friendship COLUMNS in, out UNIQUE;
