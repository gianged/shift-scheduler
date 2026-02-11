CREATE TYPE job_status AS ENUM ("PENDING","PROCESSING","COMPLETED","FAILED");
CREATE TYPE shift_type AS ENUM ("MORNING", "EVENING", "DAY_OFF");

CREATE TABLE schedule_jobs (
    id UUID CONSTRAINT pk_schedule_jobs PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL,
    run_at DATE NOT NULL,
    status job_status NOT NULL DEFAULT 'PENDING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE shift_assignments (
    id UUID CONSTRAINT pk_shift_assignments PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID CONSTRAINT fk_sa_job REFERENCES schedule_jobs(id) ON DELETE CASCADE,
    staff_id UUID NOT NULL,
    date DATE NOT NULL,
    shift_type shift_type NOT NULL
);

CREATE INDEX idx_jobs_status ON schedule_jobs(status);
CREATE INDEX idx_jobs_group on schedule_jobs(group_id);
CREATE INDEX idx_sa_job ON shift_assignments(job_id);
CREATE INDEX idx_sa_staff on shift_assignments(staff_id);