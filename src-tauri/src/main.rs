mod services;

use services::hierarchy::Database;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
fn initialize_db() -> Result<(), String> {
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
            let db = Database::new(app_data.join("edutrack.db"), &app_data)?;
            app.manage(Arc::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            initialize_db,
            services::agents::get_agent_insights,
            services::agents::run_class_agents,
            services::agents::update_class_agents,
            services::auth::can_perform_action,
            services::auth::get_role_permissions,
            services::attendance::create_attendance_record,
            services::attendance::create_session,
            services::attendance::get_all_calendar_sessions,
            services::attendance::get_attendance_by_session,
            services::attendance::get_sessions_by_class,
            services::attendance::update_session_completion,
            services::exports::export_school_data_json,
            services::exports::export_student_report_txt,
            services::exports::export_year_plan_csv,
            services::hierarchy::create_class,
            services::hierarchy::create_level,
            services::hierarchy::create_school,
            services::hierarchy::create_student,
            services::hierarchy::archive_class,
            services::hierarchy::archive_school,
            services::hierarchy::archive_student,
            services::hierarchy::update_student_note,
            services::hierarchy::get_classes_by_level,
            services::hierarchy::get_levels_by_school,
            services::hierarchy::get_schools,
            services::hierarchy::get_students_by_class,
            services::hierarchy::delete_school_data,
            services::hierarchy::get_teacher_profile,
            services::hierarchy::save_teacher_profile,
            services::hierarchy::save_image_file,
            services::hierarchy::read_image_file,
            services::llm_service::extract_curriculum_units,
            services::llm_service::get_syllabus_raw_text,
            services::llm_service::get_syllabus_extraction_diagnostics,
            services::llm_service::get_local_model_config,
            services::llm_service::list_llm_models,
            services::llm_service::set_llm_provider,
            services::planning::calendar_service::add_manual_closure_day,
            services::planning::calendar_service::create_academic_calendar,
            services::planning::calendar_service::get_calendar_closure_days,
            services::planning::calendar_service::get_calendar_stats,
            services::planning::calendar_service::get_teachable_day_dates,
            services::planning::calendar_service::get_teachable_days,
            services::planning::calendar_service::sync_public_holidays,
            services::planning::scheduling::get_class_schedule_rules,
            services::planning::scheduling::set_class_schedule_rules,
            services::planning::year_plan_generator::generate_year_plan,
            services::planning::year_plan_generator::get_year_plan,
            services::planning::year_plan_generator::get_year_plan_lessons,
            services::planning::year_plan_generator::generate_detailed_lesson_plans,
            services::planning::year_plan_generator::generate_detailed_lesson_plan_for_lesson,
            services::planning::year_plan_generator::attach_detailed_plans_to_calendar_lessons,
            services::planning::year_plan_generator::publish_year_plan,
            services::planning::year_plan_generator::update_lesson_status,
            services::planning::year_plan_generator::reschedule_missed_lessons,
            services::planning::year_plan_generator::get_all_calendar_lessons,
            services::reports::report_generator::generate_student_report,
            services::reports::report_generator::get_student_report,
            services::reports::report_generator::update_student_report,
            services::reports::report_generator::get_class_report_status,
            services::reports::report_generator::get_student_data_bundle,
            services::reports::report_generator::generate_student_report_v2,
            services::syllabus_processing::get_syllabus_for_review,
            services::syllabus_processing::run_advanced_curriculum_extraction,
            services::syllabus_processing::update_syllabus_review,
            services::syllabus_processing::upload_syllabus,
            services::tests::auto_score_submissions,
            services::tests::create_test_based_on_syllabus,
            services::tests::create_test,
            services::tests::get_grading_diagnostics,
            services::tests::install_grading_tools,
            services::tests::get_class_tests,
            services::tests::get_test_submissions,
            services::tests::score_submission,
            services::tests::upload_submission_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
