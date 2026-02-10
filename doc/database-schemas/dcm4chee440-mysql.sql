-- MySQL dump 10.13  Distrib 5.7.39, for Linux (x86_64)
--
-- Host: localhost    Database: pacsdb
-- ------------------------------------------------------
-- Server version	5.7.39-log

/*!40101 SET @OLD_CHARACTER_SET_CLIENT=@@CHARACTER_SET_CLIENT */;
/*!40101 SET @OLD_CHARACTER_SET_RESULTS=@@CHARACTER_SET_RESULTS */;
/*!40101 SET @OLD_COLLATION_CONNECTION=@@COLLATION_CONNECTION */;
/*!40101 SET NAMES utf8 */;
/*!40103 SET @OLD_TIME_ZONE=@@TIME_ZONE */;
/*!40103 SET TIME_ZONE='+00:00' */;
/*!40014 SET @OLD_UNIQUE_CHECKS=@@UNIQUE_CHECKS, UNIQUE_CHECKS=0 */;
/*!40014 SET @OLD_FOREIGN_KEY_CHECKS=@@FOREIGN_KEY_CHECKS, FOREIGN_KEY_CHECKS=0 */;
/*!40101 SET @OLD_SQL_MODE=@@SQL_MODE, SQL_MODE='NO_AUTO_VALUE_ON_ZERO' */;
/*!40111 SET @OLD_SQL_NOTES=@@SQL_NOTES, SQL_NOTES=0 */;
SET @MYSQLDUMP_TEMP_LOG_BIN = @@SESSION.SQL_LOG_BIN;
SET @@SESSION.SQL_LOG_BIN= 0;

--
-- Table structure for table `code`
--

DROP TABLE IF EXISTS `code`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `code` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `code_meaning` varchar(255) NOT NULL,
  `code_value` varchar(255) NOT NULL,
  `code_designator` varchar(255) NOT NULL,
  `code_version` varchar(255) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `code_idx` (`code_value`,`code_designator`,`code_version`)
) ENGINE=InnoDB AUTO_INCREMENT=38454 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `content_item`
--

DROP TABLE IF EXISTS `content_item`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `content_item` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `rel_type` varchar(255) NOT NULL,
  `text_value` varchar(255) DEFAULT NULL,
  `code_fk` bigint(20) DEFAULT NULL,
  `name_fk` bigint(20) DEFAULT NULL,
  `instance_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FK318FE31937EDB1AA` (`instance_fk`),
  KEY `FK318FE31970C135AA` (`code_fk`),
  KEY `FK318FE3199F40BC4C` (`name_fk`),
  KEY `content_item_rel_type_idx` (`rel_type`),
  KEY `content_item_text_value_idx` (`text_value`),
  CONSTRAINT `FK318FE31937EDB1AA` FOREIGN KEY (`instance_fk`) REFERENCES `instance` (`pk`),
  CONSTRAINT `FK318FE31970C135AA` FOREIGN KEY (`code_fk`) REFERENCES `code` (`pk`),
  CONSTRAINT `FK318FE3199F40BC4C` FOREIGN KEY (`name_fk`) REFERENCES `code` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=619522 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `dicomattrs`
--

DROP TABLE IF EXISTS `dicomattrs`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `dicomattrs` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `attrs` longblob NOT NULL,
  PRIMARY KEY (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=799656394 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `file_ref`
--

DROP TABLE IF EXISTS `file_ref`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `file_ref` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `created_time` datetime NOT NULL,
  `file_digest` varchar(255) DEFAULT NULL,
  `filepath` varchar(255) NOT NULL,
  `file_size` bigint(20) NOT NULL,
  `file_time_zone` varchar(255) DEFAULT NULL,
  `file_status` int(11) NOT NULL,
  `file_tsuid` varchar(255) NOT NULL,
  `filesystem_fk` bigint(20) DEFAULT NULL,
  `instance_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FKD42DBF5037EDB1AA` (`instance_fk`),
  KEY `FKD42DBF50206F5C8A` (`filesystem_fk`),
  CONSTRAINT `FKD42DBF50206F5C8A` FOREIGN KEY (`filesystem_fk`) REFERENCES `filesystem` (`pk`),
  CONSTRAINT `FKD42DBF5037EDB1AA` FOREIGN KEY (`instance_fk`) REFERENCES `instance` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=406416681 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `filesystem`
--

DROP TABLE IF EXISTS `filesystem`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `filesystem` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `availability` int(11) NOT NULL,
  `fs_group_id` varchar(255) NOT NULL,
  `fs_status` int(11) NOT NULL,
  `fs_uri` varchar(255) NOT NULL,
  `next_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `fs_uri` (`fs_uri`),
  KEY `FKA2455AABE9B3E742` (`next_fk`),
  KEY `fs_group_id_idx` (`fs_group_id`),
  KEY `fs_status_idx` (`fs_status`),
  CONSTRAINT `FKA2455AABE9B3E742` FOREIGN KEY (`next_fk`) REFERENCES `filesystem` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `instance`
--

DROP TABLE IF EXISTS `instance`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `instance` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `archived` bit(1) NOT NULL,
  `availability` int(11) NOT NULL,
  `sr_complete` varchar(255) NOT NULL,
  `content_date` varchar(255) NOT NULL,
  `content_time` varchar(255) NOT NULL,
  `created_time` datetime NOT NULL,
  `ext_retr_aet` varchar(255) DEFAULT NULL,
  `inst_custom1` varchar(255) NOT NULL,
  `inst_custom2` varchar(255) NOT NULL,
  `inst_custom3` varchar(255) NOT NULL,
  `inst_no` varchar(255) NOT NULL,
  `retrieve_aets` varchar(255) DEFAULT NULL,
  `sop_cuid` varchar(255) NOT NULL,
  `sop_iuid` varchar(255) NOT NULL,
  `updated_time` datetime NOT NULL,
  `sr_verified` varchar(255) NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `srcode_fk` bigint(20) DEFAULT NULL,
  `reject_code_fk` bigint(20) DEFAULT NULL,
  `series_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `inst_sop_iuid_idx` (`sop_iuid`),
  KEY `FK211694958151AFEA` (`series_fk`),
  KEY `FK2116949540F8410A` (`reject_code_fk`),
  KEY `FK211694954DC50E6B` (`srcode_fk`),
  KEY `FK2116949585AF69D8` (`dicomattrs_fk`),
  KEY `inst_sop_cuid_idx` (`sop_cuid`),
  KEY `inst_no_idx` (`inst_no`),
  KEY `inst_content_date_idx` (`content_date`),
  KEY `inst_content_time_idx` (`content_time`),
  KEY `inst_sr_verified_idx` (`sr_verified`),
  KEY `inst_sr_complete_idx` (`sr_complete`),
  KEY `inst_availability` (`availability`),
  KEY `inst_custom1_idx` (`inst_custom1`),
  KEY `inst_custom2_idx` (`inst_custom2`),
  KEY `inst_custom3_idx` (`inst_custom3`),
  CONSTRAINT `FK2116949540F8410A` FOREIGN KEY (`reject_code_fk`) REFERENCES `code` (`pk`),
  CONSTRAINT `FK211694954DC50E6B` FOREIGN KEY (`srcode_fk`) REFERENCES `code` (`pk`),
  CONSTRAINT `FK211694958151AFEA` FOREIGN KEY (`series_fk`) REFERENCES `series` (`pk`),
  CONSTRAINT `FK2116949585AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=406372352 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `issuer`
--

DROP TABLE IF EXISTS `issuer`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `issuer` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `entity_id` varchar(255) DEFAULT NULL,
  `entity_uid` varchar(255) DEFAULT NULL,
  `entity_uid_type` varchar(255) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `issuer_entity_id_idx` (`entity_id`),
  UNIQUE KEY `issuer_entity_uid_idx` (`entity_uid`,`entity_uid_type`)
) ENGINE=InnoDB AUTO_INCREMENT=1581990 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `mpps`
--

DROP TABLE IF EXISTS `mpps`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `mpps` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `accession_no` varchar(255) DEFAULT NULL,
  `created_time` datetime NOT NULL,
  `modality` varchar(255) NOT NULL,
  `station_aet` varchar(255) NOT NULL,
  `mpps_iuid` varchar(255) NOT NULL,
  `pps_start_date` varchar(255) NOT NULL,
  `pps_start_time` varchar(255) NOT NULL,
  `mpps_status` int(11) NOT NULL,
  `updated_time` datetime NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `drcode_fk` bigint(20) DEFAULT NULL,
  `patient_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `mpps_iuid_idx` (`mpps_iuid`),
  KEY `FK333EE69DC28D5C` (`drcode_fk`),
  KEY `FK333EE6A511AE1E` (`patient_fk`),
  KEY `FK333EE685AF69D8` (`dicomattrs_fk`),
  CONSTRAINT `FK333EE685AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`),
  CONSTRAINT `FK333EE69DC28D5C` FOREIGN KEY (`drcode_fk`) REFERENCES `code` (`pk`),
  CONSTRAINT `FK333EE6A511AE1E` FOREIGN KEY (`patient_fk`) REFERENCES `patient` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `mwl_item`
--

DROP TABLE IF EXISTS `mwl_item`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `mwl_item` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `accession_no` varchar(255) DEFAULT NULL,
  `created_time` datetime NOT NULL,
  `modality` varchar(255) NOT NULL,
  `req_proc_id` varchar(255) NOT NULL,
  `sps_id` varchar(255) NOT NULL,
  `sps_start_date` varchar(255) NOT NULL,
  `sps_start_time` varchar(255) NOT NULL,
  `sps_status` varchar(255) NOT NULL,
  `study_iuid` varchar(255) NOT NULL,
  `updated_time` datetime NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `patient_fk` bigint(20) DEFAULT NULL,
  `perf_phys_name_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FK8F9D3D30A511AE1E` (`patient_fk`),
  KEY `FK8F9D3D3085AF69D8` (`dicomattrs_fk`),
  KEY `FK8F9D3D30E53AEEC8` (`perf_phys_name_fk`),
  KEY `mwl_item_sps_id_idx` (`sps_id`),
  KEY `mwl_item_req_proc_id_idx` (`req_proc_id`),
  KEY `mwl_item_study_iuid_idx` (`study_iuid`),
  KEY `mwl_item_accession_no_idx` (`accession_no`),
  KEY `mwl_item_sps_status_idx` (`sps_status`),
  KEY `mwl_item_sps_start_date_idx` (`sps_start_date`),
  KEY `mwl_item_sps_start_time_idx` (`sps_start_time`),
  KEY `mwl_item_modality_idx` (`modality`),
  CONSTRAINT `FK8F9D3D3085AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`),
  CONSTRAINT `FK8F9D3D30A511AE1E` FOREIGN KEY (`patient_fk`) REFERENCES `patient` (`pk`),
  CONSTRAINT `FK8F9D3D30E53AEEC8` FOREIGN KEY (`perf_phys_name_fk`) REFERENCES `person_name` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `patient`
--

DROP TABLE IF EXISTS `patient`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `patient` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `created_time` datetime NOT NULL,
  `no_pat_id` bit(1) NOT NULL,
  `pat_birthdate` varchar(255) NOT NULL,
  `pat_custom1` varchar(255) NOT NULL,
  `pat_custom2` varchar(255) NOT NULL,
  `pat_custom3` varchar(255) NOT NULL,
  `pat_sex` varchar(255) NOT NULL,
  `updated_time` datetime NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `merge_fk` bigint(20) DEFAULT NULL,
  `pat_name_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FKD0D3EB05206840B` (`merge_fk`),
  KEY `FKD0D3EB05E7945C3` (`pat_name_fk`),
  KEY `FKD0D3EB0585AF69D8` (`dicomattrs_fk`),
  KEY `no_pat_id_idx` (`no_pat_id`),
  KEY `pat_birthdate_idx` (`pat_birthdate`),
  KEY `pat_sex_idx` (`pat_sex`),
  KEY `pat_custom1_idx` (`pat_custom1`),
  KEY `pat_custom2_idx` (`pat_custom2`),
  KEY `pat_custom3_idx` (`pat_custom3`),
  CONSTRAINT `FKD0D3EB05206840B` FOREIGN KEY (`merge_fk`) REFERENCES `patient` (`pk`),
  CONSTRAINT `FKD0D3EB0585AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`),
  CONSTRAINT `FKD0D3EB05E7945C3` FOREIGN KEY (`pat_name_fk`) REFERENCES `person_name` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=1841065 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `patient_id`
--

DROP TABLE IF EXISTS `patient_id`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `patient_id` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `pat_id` varchar(255) NOT NULL,
  `issuer_fk` bigint(20) DEFAULT NULL,
  `patient_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `pat_id_idx` (`pat_id`,`issuer_fk`),
  KEY `FK8523EC95A511AE1E` (`patient_fk`),
  KEY `FK8523EC959E0B30AA` (`issuer_fk`),
  KEY `pat_id` (`pat_id`,`patient_fk`) USING BTREE,
  CONSTRAINT `FK8523EC959E0B30AA` FOREIGN KEY (`issuer_fk`) REFERENCES `issuer` (`pk`),
  CONSTRAINT `FK8523EC95A511AE1E` FOREIGN KEY (`patient_fk`) REFERENCES `patient` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=1836777 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `person_name`
--

DROP TABLE IF EXISTS `person_name`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `person_name` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `family_name` varchar(255) DEFAULT NULL,
  `given_name` varchar(255) DEFAULT NULL,
  `i_family_name` varchar(255) DEFAULT NULL,
  `i_given_name` varchar(255) DEFAULT NULL,
  `i_middle_name` varchar(255) DEFAULT NULL,
  `i_name_prefix` varchar(255) DEFAULT NULL,
  `i_name_suffix` varchar(255) DEFAULT NULL,
  `middle_name` varchar(255) DEFAULT NULL,
  `name_prefix` varchar(255) DEFAULT NULL,
  `name_suffix` varchar(255) DEFAULT NULL,
  `p_family_name` varchar(255) DEFAULT NULL,
  `p_given_name` varchar(255) DEFAULT NULL,
  `p_middle_name` varchar(255) DEFAULT NULL,
  `p_name_prefix` varchar(255) DEFAULT NULL,
  `p_name_suffix` varchar(255) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `family_name_idx` (`family_name`),
  KEY `given_name_idx` (`given_name`),
  KEY `middle_name_idx` (`middle_name`),
  KEY `i_family_name_idx` (`i_family_name`),
  KEY `i_given_name_idx` (`i_given_name`),
  KEY `i_middle_name_idx` (`i_middle_name`),
  KEY `p_family_name_idx` (`p_family_name`),
  KEY `p_given_name_idx` (`p_given_name`),
  KEY `p_middle_name_idx` (`p_middle_name`)
) ENGINE=InnoDB AUTO_INCREMENT=8650548 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `rel_linked_patient_id`
--

DROP TABLE IF EXISTS `rel_linked_patient_id`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `rel_linked_patient_id` (
  `patient_fk` bigint(20) NOT NULL,
  `patient_id_fk` bigint(20) NOT NULL,
  KEY `FK268C10558B0E8FE9` (`patient_id_fk`),
  KEY `FK268C1055A511AE1E` (`patient_fk`),
  CONSTRAINT `FK268C10558B0E8FE9` FOREIGN KEY (`patient_id_fk`) REFERENCES `patient_id` (`pk`),
  CONSTRAINT `FK268C1055A511AE1E` FOREIGN KEY (`patient_fk`) REFERENCES `patient` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `rel_study_pcode`
--

DROP TABLE IF EXISTS `rel_study_pcode`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `rel_study_pcode` (
  `study_fk` bigint(20) NOT NULL,
  `pcode_fk` bigint(20) NOT NULL,
  KEY `FK2EF025C1E344D73A` (`pcode_fk`),
  KEY `FK2EF025C14BDB761E` (`study_fk`),
  CONSTRAINT `FK2EF025C14BDB761E` FOREIGN KEY (`study_fk`) REFERENCES `study` (`pk`),
  CONSTRAINT `FK2EF025C1E344D73A` FOREIGN KEY (`pcode_fk`) REFERENCES `code` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `series`
--

DROP TABLE IF EXISTS `series`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `series` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `availability` int(11) NOT NULL,
  `body_part` varchar(255) NOT NULL,
  `created_time` datetime NOT NULL,
  `ext_retr_aet` varchar(255) DEFAULT NULL,
  `institution` varchar(255) NOT NULL,
  `department` varchar(255) NOT NULL,
  `laterality` varchar(255) NOT NULL,
  `modality` varchar(255) NOT NULL,
  `num_instances1` int(11) NOT NULL,
  `num_instances2` int(11) NOT NULL,
  `num_instances3` int(11) NOT NULL,
  `pps_cuid` varchar(255) NOT NULL,
  `pps_iuid` varchar(255) NOT NULL,
  `pps_start_date` varchar(255) NOT NULL,
  `pps_start_time` varchar(255) NOT NULL,
  `retrieve_aets` varchar(255) DEFAULT NULL,
  `series_custom1` varchar(255) NOT NULL,
  `series_custom2` varchar(255) NOT NULL,
  `series_custom3` varchar(255) NOT NULL,
  `series_desc` varchar(255) NOT NULL,
  `series_iuid` varchar(255) NOT NULL,
  `series_no` varchar(255) NOT NULL,
  `src_aet` varchar(255) DEFAULT NULL,
  `station_name` varchar(255) NOT NULL,
  `updated_time` datetime NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `inst_code_fk` bigint(20) DEFAULT NULL,
  `perf_phys_name_fk` bigint(20) DEFAULT NULL,
  `study_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `series_iuid_idx` (`series_iuid`),
  KEY `FKCA01FE77B729CB1` (`inst_code_fk`),
  KEY `FKCA01FE774BDB761E` (`study_fk`),
  KEY `FKCA01FE7785AF69D8` (`dicomattrs_fk`),
  KEY `FKCA01FE77E53AEEC8` (`perf_phys_name_fk`),
  KEY `series_no_idx` (`series_no`),
  KEY `series_modality_idx` (`modality`),
  KEY `series_station_name_idx` (`station_name`),
  KEY `series_pps_start_date_idx` (`pps_start_date`),
  KEY `series_pps_start_time_idx` (`pps_start_time`),
  KEY `series_body_part_idx` (`body_part`),
  KEY `series_laterality_idx` (`laterality`),
  KEY `series_desc_idx` (`series_desc`),
  KEY `series_institution_idx` (`institution`),
  KEY `series_department_idx` (`department`),
  KEY `series_custom1_idx` (`series_custom1`),
  KEY `series_custom2_idx` (`series_custom2`),
  KEY `series_custom3_idx` (`series_custom3`),
  CONSTRAINT `FKCA01FE774BDB761E` FOREIGN KEY (`study_fk`) REFERENCES `study` (`pk`),
  CONSTRAINT `FKCA01FE7785AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`),
  CONSTRAINT `FKCA01FE77B729CB1` FOREIGN KEY (`inst_code_fk`) REFERENCES `code` (`pk`),
  CONSTRAINT `FKCA01FE77E53AEEC8` FOREIGN KEY (`perf_phys_name_fk`) REFERENCES `person_name` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=17891313 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `series_req`
--

DROP TABLE IF EXISTS `series_req`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `series_req` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `accession_no` varchar(255) NOT NULL,
  `req_proc_id` varchar(255) NOT NULL,
  `req_service` varchar(255) NOT NULL,
  `sps_id` varchar(255) NOT NULL,
  `study_iuid` varchar(255) NOT NULL,
  `accno_issuer_fk` bigint(20) DEFAULT NULL,
  `req_phys_name_fk` bigint(20) DEFAULT NULL,
  `series_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FKE38CD2D6C45E7AAD` (`accno_issuer_fk`),
  KEY `FKE38CD2D68151AFEA` (`series_fk`),
  KEY `FKE38CD2D633B55733` (`req_phys_name_fk`),
  KEY `series_req_accession_no_idx` (`accession_no`),
  KEY `series_req_service_idx` (`req_service`),
  KEY `series_req_proc_id_idx` (`req_proc_id`),
  KEY `series_req_sps_id_idx` (`sps_id`),
  KEY `series_req_study_iuid_idx` (`study_iuid`),
  CONSTRAINT `FKE38CD2D633B55733` FOREIGN KEY (`req_phys_name_fk`) REFERENCES `person_name` (`pk`),
  CONSTRAINT `FKE38CD2D68151AFEA` FOREIGN KEY (`series_fk`) REFERENCES `series` (`pk`),
  CONSTRAINT `FKE38CD2D6C45E7AAD` FOREIGN KEY (`accno_issuer_fk`) REFERENCES `issuer` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=8682577 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `soundex_code`
--

DROP TABLE IF EXISTS `soundex_code`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `soundex_code` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `sx_code_value` varchar(255) NOT NULL,
  `sx_pn_comp_part` int(11) NOT NULL,
  `sx_pn_comp` int(11) NOT NULL,
  `person_name_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FKA3E90A0A7665E75` (`person_name_fk`),
  KEY `sx_code_value_idx` (`sx_code_value`),
  KEY `sx_pn_comp_idx` (`sx_pn_comp`),
  KEY `sx_pn_comp_part_idx` (`sx_pn_comp_part`),
  CONSTRAINT `FKA3E90A0A7665E75` FOREIGN KEY (`person_name_fk`) REFERENCES `person_name` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=12471815 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `sps_station_aet`
--

DROP TABLE IF EXISTS `sps_station_aet`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `sps_station_aet` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `station_aet` varchar(255) NOT NULL,
  `mwl_item_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FK786E2A3CF8FD7F43` (`mwl_item_fk`),
  KEY `sps_station_aet_idx` (`station_aet`),
  CONSTRAINT `FK786E2A3CF8FD7F43` FOREIGN KEY (`mwl_item_fk`) REFERENCES `mwl_item` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `study`
--

DROP TABLE IF EXISTS `study`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `study` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `access_control_id` varchar(255) DEFAULT NULL,
  `accession_no` varchar(255) NOT NULL,
  `availability` int(11) NOT NULL,
  `created_time` datetime NOT NULL,
  `ext_retr_aet` varchar(255) DEFAULT NULL,
  `mods_in_study` varchar(255) DEFAULT NULL,
  `num_instances1` int(11) NOT NULL,
  `num_instances2` int(11) NOT NULL,
  `num_instances3` int(11) NOT NULL,
  `num_series1` int(11) NOT NULL,
  `num_series2` int(11) NOT NULL,
  `num_series3` int(11) NOT NULL,
  `retrieve_aets` varchar(255) DEFAULT NULL,
  `cuids_in_study` varchar(255) DEFAULT NULL,
  `study_custom1` varchar(255) NOT NULL,
  `study_custom2` varchar(255) NOT NULL,
  `study_custom3` varchar(255) NOT NULL,
  `study_date` varchar(255) NOT NULL,
  `study_desc` varchar(255) NOT NULL,
  `study_id` varchar(255) NOT NULL,
  `study_iuid` varchar(255) NOT NULL,
  `study_time` varchar(255) NOT NULL,
  `updated_time` datetime NOT NULL,
  `dicomattrs_fk` bigint(20) DEFAULT NULL,
  `accno_issuer_fk` bigint(20) DEFAULT NULL,
  `patient_fk` bigint(20) DEFAULT NULL,
  `ref_phys_name_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  UNIQUE KEY `study_iuid_idx` (`study_iuid`),
  KEY `FK68B0DC9C45E7AAD` (`accno_issuer_fk`),
  KEY `FK68B0DC97F2DAD5E` (`ref_phys_name_fk`),
  KEY `FK68B0DC9A511AE1E` (`patient_fk`),
  KEY `FK68B0DC985AF69D8` (`dicomattrs_fk`),
  KEY `study_id_idx` (`study_id`),
  KEY `study_date_idx` (`study_date`),
  KEY `study_time_idx` (`study_time`),
  KEY `study_accession_no_idx` (`accession_no`),
  KEY `study_desc_idx` (`study_desc`),
  KEY `study_custom1_idx` (`study_custom1`),
  KEY `study_custom2_idx` (`study_custom2`),
  KEY `study_custom3_idx` (`study_custom3`),
  KEY `study_access_control_id_idx` (`access_control_id`),
  KEY `patient_id` (`access_control_id`,`patient_fk`),
  CONSTRAINT `FK68B0DC97F2DAD5E` FOREIGN KEY (`ref_phys_name_fk`) REFERENCES `person_name` (`pk`),
  CONSTRAINT `FK68B0DC985AF69D8` FOREIGN KEY (`dicomattrs_fk`) REFERENCES `dicomattrs` (`pk`),
  CONSTRAINT `FK68B0DC9A511AE1E` FOREIGN KEY (`patient_fk`) REFERENCES `patient` (`pk`),
  CONSTRAINT `FK68B0DC9C45E7AAD` FOREIGN KEY (`accno_issuer_fk`) REFERENCES `issuer` (`pk`)
) ENGINE=InnoDB AUTO_INCREMENT=5875791 DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- Table structure for table `verify_observer`
--

DROP TABLE IF EXISTS `verify_observer`;
/*!40101 SET @saved_cs_client     = @@character_set_client */;
/*!40101 SET character_set_client = utf8 */;
CREATE TABLE `verify_observer` (
  `pk` bigint(20) NOT NULL AUTO_INCREMENT,
  `verify_datetime` varchar(255) NOT NULL,
  `instance_fk` bigint(20) DEFAULT NULL,
  `observer_name_fk` bigint(20) DEFAULT NULL,
  PRIMARY KEY (`pk`),
  KEY `FKC9DB73DC37EDB1AA` (`instance_fk`),
  KEY `FKC9DB73DC661F04F6` (`observer_name_fk`),
  KEY `vo_verify_datetime_idx` (`verify_datetime`),
  CONSTRAINT `FKC9DB73DC37EDB1AA` FOREIGN KEY (`instance_fk`) REFERENCES `instance` (`pk`),
  CONSTRAINT `FKC9DB73DC661F04F6` FOREIGN KEY (`observer_name_fk`) REFERENCES `person_name` (`pk`)
) ENGINE=InnoDB DEFAULT CHARSET=latin1;
/*!40101 SET character_set_client = @saved_cs_client */;

--
-- GTID state at the end of the backup 
--

SET @@GLOBAL.GTID_PURGED='b9461621-c546-11ed-9918-005056010d69:1-28858661';
/*!40103 SET TIME_ZONE=@OLD_TIME_ZONE */;

/*!40101 SET SQL_MODE=@OLD_SQL_MODE */;
/*!40014 SET FOREIGN_KEY_CHECKS=@OLD_FOREIGN_KEY_CHECKS */;
/*!40014 SET UNIQUE_CHECKS=@OLD_UNIQUE_CHECKS */;
/*!40101 SET CHARACTER_SET_CLIENT=@OLD_CHARACTER_SET_CLIENT */;
/*!40101 SET CHARACTER_SET_RESULTS=@OLD_CHARACTER_SET_RESULTS */;
/*!40101 SET COLLATION_CONNECTION=@OLD_COLLATION_CONNECTION */;
/*!40111 SET SQL_NOTES=@OLD_SQL_NOTES */;

-- Dump completed on 2023-08-22 11:06:21
