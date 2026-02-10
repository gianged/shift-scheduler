CREATE TYPE staff_status AS ENUM ('ACTIVE', 'INACTIVE');

CREATE TABLE staff (
    id UUID CONSTRAINT pk_staff PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) CONSTRAINT uq_staff_email UNIQUE NOT NULL,
    position VARCHAR(255) NOT NULL,
    status staff_status NOT NULL DEFAULT 'ACTIVE',
    create_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    update_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE staff_groups (
    id UUID CONSTRAINT pk_staff_groups PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    parent_group_id UUID CONSTRAINT fk_staff_groups_parent REFERENCES staff_groups(id) ON DELETE SET NULL,
    create_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    update_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE group_memberships (
    staff_id UUID CONSTRAINT fk_gm_staff REFERENCES staff(id) ON DELETE CASCADE,
    group_id UUID CONSTRAINT fk_gm_group REFERENCES staff_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (staff_id, group_id)
);

CREATE INDEX idx_staff_status ON staff(status);
CREATE INDEX idx_staff_groups_parent on staff_groups(parent_group_id);
CREATE INDEX idx_gm_group ON group_memberships(group_id);