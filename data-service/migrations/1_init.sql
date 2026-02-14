CREATE TYPE staff_status AS ENUM(
    'ACTIVE',
    'INACTIVE'
);

CREATE TABLE staff(
    id uuid CONSTRAINT pk_staff PRIMARY KEY DEFAULT gen_random_uuid(),
    name varchar(255) NOT NULL,
    email varchar(255) CONSTRAINT uq_staff_email UNIQUE NOT NULL,
    position varchar(255) NOT NULL,
    status staff_status NOT NULL DEFAULT 'ACTIVE',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE staff_groups(
    id uuid CONSTRAINT pk_staff_groups PRIMARY KEY DEFAULT gen_random_uuid(),
    name varchar(255) NOT NULL,
    parent_group_id uuid CONSTRAINT fk_staff_groups_parent REFERENCES staff_groups(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE group_memberships(
    staff_id uuid CONSTRAINT fk_gm_staff REFERENCES staff(id) ON DELETE CASCADE,
    group_id uuid CONSTRAINT fk_gm_group REFERENCES staff_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (staff_id, group_id)
);

CREATE INDEX idx_staff_status ON staff(status);

CREATE INDEX idx_staff_groups_parent ON staff_groups(parent_group_id);

CREATE INDEX idx_gm_group ON group_memberships(group_id);

CREATE INDEX idx_gm_staff ON group_memberships(staff_id);
