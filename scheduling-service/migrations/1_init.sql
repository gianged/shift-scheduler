CREATE TYPE job_status AS ENUM(
    'PENDING',
    'PROCESSING',
    'COMPLETED',
    'FAILED'
);

CREATE TYPE shift_type AS ENUM(
    'MORNING',
    'EVENING',
    'DAY_OFF'
);

CREATE TABLE schedule_jobs(
    id uuid CONSTRAINT pk_schedule_jobs PRIMARY KEY DEFAULT gen_random_uuid(),
    staff_group_id uuid NOT NULL,
    period_begin_date date NOT NULL,
    status job_status NOT NULL DEFAULT 'PENDING',
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE shift_assignments(
    id uuid CONSTRAINT pk_shift_assignments PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id uuid NOT NULL CONSTRAINT fk_sa_job REFERENCES schedule_jobs(id) ON DELETE CASCADE,
    staff_id uuid NOT NULL,
    date date NOT NULL,
    shift_type shift_type NOT NULL
);

CREATE INDEX idx_jobs_status ON schedule_jobs(status);

CREATE INDEX idx_jobs_group ON schedule_jobs(staff_group_id);

CREATE INDEX idx_sa_job ON shift_assignments(job_id);

CREATE INDEX idx_sa_staff ON shift_assignments(staff_id);
