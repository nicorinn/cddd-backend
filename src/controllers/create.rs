use actix_web::{http::header::ContentType, post, web, HttpResponse, Responder};
use sqlx::{query, types::Uuid, Error, PgPool};

use crate::{
    controllers::utils::convert_sqlx_error,
    models::create_model::{CreateModel, Target},
};

#[post("/create")]
async fn create(
    pool_data: web::Data<PgPool>,
    compound_json: web::Json<CreateModel>,
) -> impl Responder {
    let model = compound_json.0;

    let pool = &pool_data.as_ref();

    // add compound √
    // add compound names √
    // add indications (including indication types)
    // add MOA √
    // add diseases √
    // add pathway annotations √
    // add targets (including target names) √
    // add gene targets √
    // add clinical annotations √

    // REPURPOSING
    // add repurposing compound name √
    // add repurposing √
    // add repurposing efforts √
    // add repurposing indications √

    // join compound and repurposing

    let compound_id = add_compound(pool, model.clone())
        .await
        .map_err(convert_sqlx_error)
        .unwrap();

    let compound_name_ids = add_compound_names(pool, model.names.clone())
        .await
        .map_err(convert_sqlx_error)
        .unwrap();

    // Create join table for compound and compound_name
    for compound_name_id in compound_name_ids {
        create_join(
            pool,
            "compound_name",
            "compound",
            compound_name_id,
            compound_id,
        )
        .await
        .unwrap();
    }

    // Insert indications

    let indication_ids = add_indications(
        pool,
        model.clone().indications,
        Some(model.indication_type.clone()),
    )
    .await
    .unwrap();

    for indication_id in indication_ids.clone() {
        create_join(pool, "compound", "indication", compound_id, indication_id)
            .await
            .unwrap();
    }

    // Mechanism of action

    let moa_ids = add_input_vec(
        pool,
        model.mechanism_of_action.clone(),
        "mechanism_of_action".to_string(),
        "mechanism".to_string(),
    )
    .await
    .unwrap();

    for moa_id in moa_ids {
        create_join(pool, "compound", "mechanism_of_action", compound_id, moa_id)
            .await
            .unwrap();
    }

    // Diseases

    let disease_ids = add_input_vec(
        pool,
        model.diseases.clone(),
        "disease".to_string(),
        "name".to_string(),
    )
    .await
    .unwrap();

    for disease_id in disease_ids {
        create_join(pool, "compound", "disease", compound_id, disease_id)
            .await
            .unwrap();
    }

    // Pathway Annotations

    let pathway_annotation_ids = add_input_vec(
        pool,
        model.pathway_annotation.clone(),
        "pathway_annotation".to_string(),
        "annotation".to_string(),
    )
    .await
    .unwrap();

    for p_id in pathway_annotation_ids {
        create_join(pool, "compound", "pathway_annotation", compound_id, p_id)
            .await
            .unwrap();
    }

    // Clinical Annotations

    let clinical_annotation_ids = add_input_vec(
        pool,
        model.clinical_annotation.clone(),
        "clinical_annotation".to_string(),
        "annotation".to_string(),
    )
    .await
    .unwrap();

    for clin_id in clinical_annotation_ids {
        create_join(
            pool,
            "compound",
            "clinical_annotation",
            compound_id,
            clin_id,
        )
        .await
        .unwrap();
    }

    // Gene/Gene Targets

    let gene_ids = add_input_vec(
        pool,
        model.genes.clone(),
        "gene_target".to_string(),
        "gene".to_string(),
    )
    .await
    .unwrap();

    for gene_id in gene_ids {
        create_join(pool, "compound", "gene_target", compound_id, gene_id)
            .await
            .unwrap();
    }

    // Targets

    let target_ids = add_targets(pool, model.targets.clone()).await.unwrap();

    for target_id in target_ids {
        create_join(pool, "compound", "target", compound_id, target_id)
            .await
            .unwrap();
    }

    // Reporposing

    if !model.repurposed_efforts.is_none() {
        let repurposing_id = add_repurposing(pool, model.clone(), compound_id)
            .await
            .unwrap();
        let repurposing_indication_ids = add_indications(pool, model.indications.clone(), None)
            .await
            .unwrap();

        for r_indication_id in repurposing_indication_ids {
            create_join(
                pool,
                "repurposing",
                "indication",
                repurposing_id,
                r_indication_id,
            )
            .await
            .unwrap();
        }

        create_join(pool, "compound", "repurposing", compound_id, repurposing_id)
            .await
            .unwrap();
    }

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(indication_ids.len().to_string())
}

/** Insert each compound name into the DB */
async fn add_compound_names(pool: &PgPool, names: Vec<String>) -> Result<Vec<Uuid>, Error> {
    let insert_query = "insert into compound_name (name, is_repurposed) values ($1, false)
                            returning id new_id;";
    let mut compound_name_ids: Vec<Uuid> = vec![];
    for compound_name in names {
        let new_id: Uuid = sqlx::query_scalar(insert_query)
            .bind(&compound_name)
            .fetch_one(pool)
            .await?;
        compound_name_ids.push(new_id);
    }

    Ok(compound_name_ids)
}

/** Retrieve or insert each indication. */
async fn add_indications(
    pool: &PgPool,
    indications: Vec<String>,
    indication_type: Option<String>,
) -> Result<Vec<Uuid>, Error> {
    let search_query = "select id from indication where indication ilike $1;";
    let insert_query = "insert into indication (indication, type) values ($1, $2)
                            returning id new_id;";
    let mut indication_ids: Vec<Uuid> = vec![];
    for indication in indications {
        // First check if indication exists
        let existing_id: Option<Uuid> = sqlx::query_scalar(search_query)
            .bind(&indication)
            .fetch_optional(pool)
            .await?;

        match existing_id {
            Some(id) => indication_ids.push(id),
            None => {
                // If it doesn't exist, add it
                let new_id: Uuid = sqlx::query_scalar(insert_query)
                    .bind(&indication)
                    .bind(indication_type.clone())
                    .fetch_one(pool)
                    .await?;
                indication_ids.push(new_id);
            }
        }
    }

    Ok(indication_ids)
}

/** Retrieve or insert each target. */
async fn add_targets(pool: &PgPool, targets: Vec<Target>) -> Result<Vec<Uuid>, Error> {
    let search_query = "select id from target where array_to_string(names, ', ') ilike '%$1%'";
    let insert_query = "insert into target (names) values ($1::varchar[]) returning id new_id;";
    let mut target_ids: Vec<Uuid> = vec![];
    for target in targets {
        // First check if target exists
        let existing_id: Option<Uuid> = sqlx::query_scalar(search_query)
            .bind(&target.names)
            .fetch_optional(pool)
            .await?;
        match existing_id {
            Some(id) => target_ids.push(id),
            None => {
                // This code is hideous but it runs rarely
                let names = serde_json::to_string(&target.names)
                    .unwrap()
                    .replace("[", "{")
                    .replace("]", "}");

                println!("{}", names);

                let new_id: Uuid = sqlx::query_scalar(insert_query)
                    .bind(names)
                    .fetch_one(pool)
                    .await?;
                target_ids.push(new_id);
            }
        }
    }

    Ok(target_ids)
}

/** Retrieve or insert each input in a vec. */
async fn add_input_vec(
    pool: &PgPool,
    inputs: Vec<String>,
    table: String,
    column: String,
) -> Result<Vec<Uuid>, Error> {
    let search_query = format!("select id from {table} where {column} ilike $1;");
    let insert_query = format!(
        "insert into {table} ({column}) values ($1)
                            returning id new_id;"
    );
    let mut ids: Vec<Uuid> = vec![];
    for input in inputs {
        // First check if indication exists
        let existing_id: Option<Uuid> = sqlx::query_scalar(&search_query)
            .bind(&input)
            .fetch_optional(pool)
            .await?;
        match existing_id {
            Some(id) => ids.push(id),
            None => {
                // If it doesn't exist, add it
                let new_id: Uuid = sqlx::query_scalar(&insert_query)
                    .bind(&input)
                    .fetch_one(pool)
                    .await?;
                ids.push(new_id);
            }
        }
    }

    Ok(ids)
}

/** Insert new compound (including company) in DB */
async fn add_compound(pool: &PgPool, model: CreateModel) -> Result<Uuid, Error> {
    let insert_query = "insert into compound 
        ( discontinuation_phase
        , discontinuation_reason
        , discontinuation_year
        , discontinuation_company_id
        , link) 
        values ($1::integer, $2, $3::integer, $4, $5)
        returning id new_id;";

    let company_id = add_company(pool, model.company).await?;

    let new_id: Uuid = sqlx::query_scalar(insert_query)
        .bind(&model.phase)
        .bind(&model.discontinuation_reason)
        .bind(&model.year_discontinued)
        .bind(&company_id)
        .bind(&model.link)
        .fetch_one(pool)
        .await?;
    Ok(new_id)
}

/** Insert new repurposing for compound (including company) in DB */
async fn add_repurposing(
    pool: &PgPool,
    model: CreateModel,
    compound_id: Uuid,
) -> Result<Uuid, Error> {
    let insert_query = "insert into repurposing 
        ( compound_id
        , compound_name_id
        , company_id
        , year
        , phase
        , efforts) 
        values ($1, $2, $3, $4::integer, $5::integer, $6)
        returning id new_id;";

    // Add company leading repurposing, if applicable
    let mut company_id = None;
    if let Some(company) = model.repurposed_company.clone() {
        company_id = Some(add_company(pool, company).await?);
    }
    // Add repurposed drug name, if applicable
    let mut compound_name_id = None;
    if let Some(drug_name) = model.repurposed_drug_name {
        let compound_name_ids = add_compound_names(pool, vec![drug_name]).await?;
        compound_name_id = Some(compound_name_ids[0]);
    }

    let new_id: Uuid = sqlx::query_scalar(insert_query)
        .bind(&compound_id)
        .bind(&compound_name_id)
        .bind(&company_id)
        .bind(&model.repurposed_year)
        .bind(&model.repurposed_phase)
        .bind(&model.repurposed_efforts)
        .fetch_one(pool)
        .await?;
    Ok(new_id)
}

/** Retrieve existing company id or add new company and return id */
async fn add_company(pool: &PgPool, company_name: String) -> Result<Uuid, Error> {
    let search_query = "select id from company where name ilike $1;";

    let existing_company_id: Option<Uuid> = sqlx::query_scalar(search_query)
        .bind(company_name.clone())
        .fetch_optional(pool)
        .await?;
    // Return existing company id if it exists
    if let Some(company_id) = existing_company_id {
        return Ok(company_id);
    }
    // If not, insert new company
    let insert_query = "insert into company (name) values ($1) returning id new_id;";
    let new_id: Uuid = sqlx::query_scalar(insert_query)
        .bind(company_name)
        .fetch_one(pool)
        .await?;

    Ok(new_id)
}

async fn create_join(
    pool: &PgPool,
    table1: &str,
    table2: &str,
    id1: Uuid,
    id2: Uuid,
) -> Result<(), Error> {
    let search_query = format!(
        "select {table1}_id from {table1}_{table2} where {table1}_id = ($1) and {table2}_id = ($2);"
    );
    let insert_query =
        format!("insert into {table1}_{table2} ({table1}_id, {table2}_id) values ($1, $2);");

    let existing_row = sqlx::query(&search_query)
        .bind(id1)
        .bind(id2)
        .fetch_optional(pool)
        .await?;

    if existing_row.is_some() {
        return Ok(());
    }

    sqlx::query(&insert_query)
        .bind(id1)
        .bind(id2)
        .execute(pool)
        .await?;

    Ok(())
}
